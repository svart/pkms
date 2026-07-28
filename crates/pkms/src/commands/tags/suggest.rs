use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use pkms_org::{OrgSnapshot, org_edit, parser};
use pkms_rag::{
    TagRecommendation, TagRecommendationRequest, TagScope, TagSourceRange, tag_query_text,
};
use pkms_task::{TaskClock, TaskModifierSpec, all_task_entries, mod_pkms_task};
use serde::Serialize;

use crate::{
    cli::{OutputFormat, TagSuggestArgs, TagSuggestionOptions},
    command_context::CommandContext,
};

use crate::commands::rag::{resolve_embedding_provider_config, resolve_rag_db};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TagTargetKind {
    Note,
    Task,
}

#[derive(Debug, Clone, Serialize)]
struct TagTargetOutput {
    kind: TagTargetKind,
    id: String,
    uuid: String,
    title: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_number: Option<usize>,
}

#[derive(Debug, Serialize)]
struct TagCommandOutput {
    target: TagTargetOutput,
    existing_tags: Vec<String>,
    suggestions: Vec<TagRecommendation>,
    applied: bool,
    rag_db: String,
    embedding_model: String,
}

struct ResolvedTagTarget {
    output: TagTargetOutput,
    scope: TagScope,
    target_range: Option<TagSourceRange>,
    existing_tags: Vec<String>,
    query: String,
    path: PathBuf,
    task_id: Option<usize>,
}

#[derive(Serialize)]
struct TagRecommendationLine<'a> {
    target: &'a TagTargetOutput,
    existing_tags: &'a [String],
    suggestion: &'a TagRecommendation,
    applied: bool,
    rag_db: &'a str,
    embedding_model: &'a str,
}

pub(super) fn run(ctx: &CommandContext<'_>, args: &TagSuggestArgs) -> Result<()> {
    execute(ctx, resolve_target(ctx, &args.target)?, &args.options)
}

fn execute(
    ctx: &CommandContext<'_>,
    target: ResolvedTagTarget,
    options: &TagSuggestionOptions,
) -> Result<()> {
    let db_path = resolve_rag_db(options.rag_db.as_ref(), ctx.config());
    let provider_config = resolve_embedding_provider_config(ctx.config())?;
    let provider = pkms_rag::provider_from_config(&provider_config)?;
    let request = TagRecommendationRequest {
        query: target.query.clone(),
        scope: target.scope,
        target_note_id: target.output.uuid.clone(),
        target_range: target.target_range,
        existing_tags: target.existing_tags.clone(),
        limit: options.limit,
        neighbor_limit: options.neighbors,
    };
    let suggestions =
        pkms_rag::RagIndex::open(&db_path)?.recommend_tags(&request, provider.as_ref())?;
    let applied = if options.apply && !suggestions.is_empty() {
        apply_tags(ctx, &target, &suggestions)?
    } else {
        false
    };
    let output = TagCommandOutput {
        target: target.output,
        existing_tags: target.existing_tags,
        suggestions,
        applied,
        rag_db: db_path.display().to_string(),
        embedding_model: provider.model_name().to_string(),
    };
    render(ctx, &output)
}

fn resolve_target(ctx: &CommandContext<'_>, target: &str) -> Result<ResolvedTagTarget> {
    if !target.is_empty() && target.bytes().all(|byte| byte.is_ascii_digit()) {
        let canonical_id = target.parse::<usize>()?;
        if canonical_id > 0 && canonical_id.to_string() == target {
            return resolve_task_target(ctx, canonical_id);
        }
    }
    let uuid = uuid::Uuid::parse_str(target)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string().eq_ignore_ascii_case(target));
    if let Some(uuid) = uuid {
        return resolve_note_target(ctx, &uuid.hyphenated().to_string());
    }
    bail!("Tag suggestion target must be a full note UUID or canonical task ID such as 12")
}

fn resolve_note_target(ctx: &CommandContext<'_>, target: &str) -> Result<ResolvedTagTarget> {
    let snapshot = load_snapshot(ctx)?;
    let node = snapshot.graph().resolve_target(target)?;
    let result = snapshot
        .corpus()
        .results()
        .iter()
        .find(|result| result.path == node.path)
        .with_context(|| format!("No parsed data for note {}", node.path.display()))?;
    let uuid = result
        .parsed
        .uuids
        .first()
        .context("Tag recommendations require a note-level ID")?
        .to_string();
    let title = result
        .parsed
        .title
        .clone()
        .unwrap_or_else(|| node.title.clone());
    let content = result
        .raw_content
        .as_deref()
        .context("Tag recommendations require readable note content")?;
    Ok(ResolvedTagTarget {
        output: TagTargetOutput {
            kind: TagTargetKind::Note,
            id: uuid.clone(),
            uuid,
            title: title.clone(),
            path: result.path.display().to_string(),
            line_number: None,
        },
        scope: TagScope::Note,
        target_range: None,
        existing_tags: result.parsed.filetags.clone(),
        query: tag_query_text(&title, content),
        path: result.path.clone(),
        task_id: None,
    })
}

fn resolve_task_target(ctx: &CommandContext<'_>, canonical_id: usize) -> Result<ResolvedTagTarget> {
    let snapshot = load_snapshot(ctx)?;
    let task_states = ctx.config().task_state_config();
    let entry = all_task_entries(&task_states, snapshot.graph())
        .into_iter()
        .find(|entry| entry.id == canonical_id)
        .with_context(|| format!("No task with canonical ID {canonical_id}"))?;
    let path = PathBuf::from(&entry.path);
    let result = snapshot
        .corpus()
        .results()
        .iter()
        .find(|result| result.path == path)
        .with_context(|| format!("No parsed data for task path {}", path.display()))?;
    let heading = result
        .parsed
        .headings
        .iter()
        .find(|heading| heading.line_number == entry.line_number)
        .context("Resolved task is no longer an org heading")?;
    let content = result
        .raw_content
        .as_deref()
        .context("Tag recommendations require readable task content")?;
    let lines = content.lines().collect::<Vec<_>>();
    let end_index =
        org_edit::parsed_heading_subtree_end_index(&result.parsed.headings, heading, lines.len());
    let subtree = lines[heading.line_number.saturating_sub(1)..end_index].join("\n");
    let uuid = result
        .parsed
        .uuids
        .first()
        .context("Task tag recommendations require a note-level ID")?
        .to_string();
    Ok(ResolvedTagTarget {
        output: TagTargetOutput {
            kind: TagTargetKind::Task,
            id: canonical_id.to_string(),
            uuid: uuid.clone(),
            title: parser::strip_org_links(&heading.title),
            path: path.display().to_string(),
            line_number: Some(heading.line_number),
        },
        scope: TagScope::Heading,
        target_range: Some(TagSourceRange {
            start_line: u32::try_from(heading.line_number).context("task line exceeds u32")?,
            end_line: u32::try_from(end_index).context("task end line exceeds u32")?,
        }),
        existing_tags: heading.tags.clone(),
        query: tag_query_text(&heading.title, &subtree),
        path,
        task_id: Some(canonical_id),
    })
}

fn load_snapshot(ctx: &CommandContext<'_>) -> Result<OrgSnapshot> {
    let config = ctx.config().org_config();
    OrgSnapshot::load(&config.scan_config(), &config.link_resolution_context())
}

fn apply_tags(
    ctx: &CommandContext<'_>,
    target: &ResolvedTagTarget,
    suggestions: &[TagRecommendation],
) -> Result<bool> {
    let mut tags = target.existing_tags.clone();
    tags.extend(suggestions.iter().map(|suggestion| suggestion.tag.clone()));
    tags.sort();
    tags.dedup();
    match target.task_id {
        None => org_edit::update_filetags(&target.path, &tags),
        Some(task_id) => {
            let spec = TaskModifierSpec {
                labels: Some(tags),
                ..TaskModifierSpec::default()
            };
            Ok(mod_pkms_task(
                &ctx.config().pkms_task_config(),
                task_id,
                &spec,
                TaskClock::now(),
            )?
            .changed)
        }
    }
}

fn render(ctx: &CommandContext<'_>, output: &TagCommandOutput) -> Result<()> {
    match ctx.output().format {
        OutputFormat::Json => ctx.output().print_json(output),
        OutputFormat::Ndjson => {
            for suggestion in &output.suggestions {
                ctx.output().print_json_line(&TagRecommendationLine {
                    target: &output.target,
                    existing_tags: &output.existing_tags,
                    suggestion,
                    applied: output.applied,
                    rag_db: &output.rag_db,
                    embedding_model: &output.embedding_model,
                })?;
            }
            Ok(())
        }
        OutputFormat::Text => {
            println!("Target: {} ({})", output.target.title, output.target.id);
            println!(
                "Existing tags: {}",
                if output.existing_tags.is_empty() {
                    "none".to_string()
                } else {
                    output.existing_tags.join(", ")
                }
            );
            if output.suggestions.is_empty() {
                println!("No tag recommendations.");
            } else {
                println!("Recommendations:");
                for (index, suggestion) in output.suggestions.iter().enumerate() {
                    let sources = suggestion
                        .evidence
                        .iter()
                        .map(|evidence| evidence.title.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!(
                        "  {}. {} ({:.3}, {} source(s): {})",
                        index + 1,
                        suggestion.tag,
                        suggestion.score,
                        suggestion.support,
                        sources
                    );
                }
                if output.applied {
                    println!("Applied {} tag(s).", output.suggestions.len());
                } else {
                    println!("Preview only; pass --apply to add these tags.");
                }
            }
            Ok(())
        }
    }
}
