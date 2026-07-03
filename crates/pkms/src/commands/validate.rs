use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::graph::validation::{DuplicateUuidIssueKind, NoteValidationIssue, SelfLinkKind};
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;
use std::path::Path;

#[derive(Serialize)]
pub struct ValidateOutput {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub headings: usize,
    pub heading_uuids: Vec<String>,
    pub outgoing: usize,
    pub incoming: usize,
    pub outgoing_internal: usize,
    pub broken_internal: Vec<String>,
    pub broken_files: Vec<String>,
    pub backlinks: Vec<BacklinkEntry>,
    pub issues: Vec<String>,
    pub healthy: bool,
}

#[derive(Serialize)]
pub struct BacklinkEntry {
    pub uuid: String,
    pub title: String,
}

impl From<&crate::graph::Node> for BacklinkEntry {
    fn from(n: &crate::graph::Node) -> Self {
        BacklinkEntry {
            uuid: n.uuid.to_string(),
            title: n.title.clone(),
        }
    }
}

fn build_validate_output(
    node: &crate::graph::Node,
    incoming_len: usize,
    broken_internal: Vec<String>,
    broken_files: Vec<String>,
    backlink_entries: Vec<BacklinkEntry>,
    issues: Vec<String>,
) -> ValidateOutput {
    let healthy = issues.is_empty();
    ValidateOutput {
        uuid: node.uuid.to_string(),
        title: node.title.clone(),
        path: node.path.display().to_string(),
        filetags: node.filetags.clone(),
        categories: node.categories.clone(),
        aliases: node.aliases.clone(),
        refs: node.refs.clone(),
        headings: node.headings_count,
        heading_uuids: node
            .heading_uuids
            .iter()
            .map(|uuid| uuid.to_string())
            .collect(),
        outgoing: node.outgoing.len(),
        incoming: incoming_len,
        outgoing_internal: node
            .outgoing
            .iter()
            .filter(|l| matches!(l, pkms_org::parser::Link::Internal(_)))
            .count(),
        broken_internal,
        broken_files,
        backlinks: backlink_entries,
        issues,
        healthy,
    }
}

fn validation_issue_message(issue: &NoteValidationIssue) -> String {
    match issue {
        NoteValidationIssue::InvalidUuidFormat { uuid } => {
            format!("Invalid UUID format: {uuid}")
        }
        NoteValidationIssue::MissingTitle => "Missing #+title: property".to_string(),
        NoteValidationIssue::InvalidFiletagsFormat { raw, reason } => {
            format!("Invalid filetags format '{raw}': {reason}")
        }
        NoteValidationIssue::DuplicateUuid { uuid, kind } => match kind {
            DuplicateUuidIssueKind::HeadingMatchesPrimary => {
                format!("Duplicate UUID: heading-level :ID: {uuid} matches the note's primary :ID:")
            }
            DuplicateUuidIssueKind::HeadingRepeatedInNote => format!(
                "Duplicate UUID: heading-level :ID: {uuid} is used by multiple headings in this note"
            ),
            DuplicateUuidIssueKind::HeadingBelongsToAnotherNote { other_paths } => format!(
                "Duplicate UUID: heading-level :ID: {uuid} belongs to another note ({})",
                other_paths.join(", ")
            ),
        },
        NoteValidationIssue::SelfLink {
            link_type,
            target,
            suggested_uuid,
        } => match (link_type, suggested_uuid) {
            (SelfLinkKind::Id, _) => format!("Self-link via id link: {target}"),
            (SelfLinkKind::File, Some(uuid)) => {
                format!("File link to own file; consider using id:{uuid} instead of file:{target}")
            }
            (SelfLinkKind::File, None) => format!("Self-link via file link to own file: {target}"),
        },
        NoteValidationIssue::Overlink {
            target_uuid,
            target_title,
            count,
        } => format!(
            "Overlinking: {count} links to \"{target_title}\" ({target_uuid}) — consider removing duplicate links"
        ),
    }
}

fn validate_one(graph: &Graph, target: &str, db_root: &Path) -> Result<ValidateOutput> {
    let node = graph.resolve_target(target)?.clone();
    validate_node(graph, &node, target, db_root)
}

fn validate_node(
    graph: &Graph,
    node: &crate::graph::Node,
    target: &str,
    db_root: &Path,
) -> Result<ValidateOutput> {
    let validation = graph.collect_node_validation_issues(node, target, db_root);
    let mut issues: Vec<String> = validation
        .issues
        .iter()
        .map(validation_issue_message)
        .collect();
    let broken_internal: Vec<String> = validation
        .broken_internal_links
        .iter()
        .map(|issue| issue.target_uuid.to_string())
        .collect();
    let broken_files: Vec<String> = validation
        .broken_file_links
        .iter()
        .map(|issue| issue.target_path.to_string())
        .collect();

    let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();

    let backlink_entries: Vec<BacklinkEntry> = incoming
        .iter()
        .filter_map(|uuid| graph.nodes.get(uuid).map(BacklinkEntry::from))
        .collect();

    if !broken_internal.is_empty() {
        issues.push(format!("{} broken internal link(s)", broken_internal.len()));
    }
    if !broken_files.is_empty() {
        issues.push(format!("{} broken file link(s)", broken_files.len()));
    }

    Ok(build_validate_output(
        node,
        incoming.len(),
        broken_internal,
        broken_files,
        backlink_entries,
        issues,
    ))
}

pub struct ValidateOptions {
    pub targets: Vec<String>,
}

pub fn run(ctx: &CommandContext<'_>, opts: &ValidateOptions) -> Result<()> {
    let outputs = execute(ctx.config(), opts)?;
    render(ctx.output(), &outputs)
}

pub fn execute(config: &ResolvedConfig, opts: &ValidateOptions) -> Result<Vec<ValidateOutput>> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    opts.targets
        .iter()
        .map(|target| validate_one(&graph, target, db_root))
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[ValidateOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", render_text(outputs));
        }
        OutputFormat::Json => {
            ctx.print_json_adaptive(outputs)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(outputs)?;
        }
    }

    Ok(())
}

pub fn render_text(outputs: &[ValidateOutput]) -> String {
    outputs
        .iter()
        .map(render_one_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one_text(output: &ValidateOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "Note: {}", output.title);
    let _ = writeln!(text, "  UUID:   {}", output.uuid);
    let _ = writeln!(text, "  Path:   {}", output.path);
    if !output.filetags.is_empty() {
        let _ = writeln!(text, "  Tags:   {}", output.filetags.join(", "));
    }
    if !output.categories.is_empty() {
        let _ = writeln!(text, "  Cats:   {}", output.categories.join(", "));
    }
    if !output.aliases.is_empty() {
        let _ = writeln!(text, "  Aliases: {}", output.aliases.join(", "));
    }
    if !output.refs.is_empty() {
        let _ = writeln!(text, "  Refs:   {}", output.refs.join(", "));
    }
    let _ = writeln!(text, "  Headings: {}", output.headings);
    if !output.heading_uuids.is_empty() {
        let _ = writeln!(text, "  Heading UUIDs: {}", output.heading_uuids.join(", "));
    }
    text.push('\n');
    text.push_str("Links:\n");
    let _ = writeln!(
        text,
        "  Outgoing: {} ({} internal)",
        output.outgoing, output.outgoing_internal,
    );
    let _ = writeln!(text, "  Incoming: {}", output.incoming);
    let _ = writeln!(
        text,
        "  Broken:   {} internal, {} file",
        output.broken_internal.len(),
        output.broken_files.len()
    );

    if !output.broken_internal.is_empty() {
        text.push('\n');
        text.push_str("Broken internal links:\n");
        for uuid in &output.broken_internal {
            let _ = writeln!(text, "  -> {uuid}");
        }
    }

    if !output.broken_files.is_empty() {
        text.push('\n');
        text.push_str("Broken file links:\n");
        for path in &output.broken_files {
            let _ = writeln!(text, "  -> {path}");
        }
    }

    if !output.issues.is_empty() {
        text.push('\n');
        for issue in &output.issues {
            let _ = writeln!(text, "Issue: {issue}");
        }
    }

    text.push('\n');
    if output.healthy {
        text.push_str("Status: healthy\n");
    } else {
        let _ = writeln!(text, "Status: {} issue(s)", output.issues.len());
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_output() -> ValidateOutput {
        ValidateOutput {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: "Note A".to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            categories: vec!["cat".to_string()],
            aliases: vec!["Alias A".to_string()],
            refs: vec!["ref-a".to_string()],
            headings: 2,
            heading_uuids: vec!["22222222-2222-4222-8222-222222222222".to_string()],
            outgoing: 3,
            incoming: 1,
            outgoing_internal: 2,
            broken_internal: vec![],
            broken_files: vec![],
            backlinks: vec![],
            issues: vec![],
            healthy: true,
        }
    }

    #[test]
    fn renders_healthy_validate_text_from_typed_output() {
        let text = render_text(&[healthy_output()]);

        assert!(text.contains("Note: Note A"));
        assert!(text.contains("  Tags:   tag"));
        assert!(text.contains("  Heading UUIDs: 22222222-2222-4222-8222-222222222222"));
        assert!(text.contains("  Outgoing: 3 (2 internal)"));
        assert!(text.ends_with("Status: healthy\n"));
    }

    #[test]
    fn renders_unhealthy_validate_text_from_typed_output() {
        let mut output = healthy_output();
        output.broken_internal = vec!["missing-id".to_string()];
        output.broken_files = vec!["missing.org".to_string()];
        output.issues = vec!["1 broken internal link(s)".to_string()];
        output.healthy = false;

        let text = render_text(&[output]);

        assert!(text.contains("Broken internal links:\n  -> missing-id"));
        assert!(text.contains("Broken file links:\n  -> missing.org"));
        assert!(text.contains("Issue: 1 broken internal link(s)"));
        assert!(text.ends_with("Status: 1 issue(s)\n"));
    }

    #[test]
    fn validate_allows_unique_heading_uuid_in_same_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("alpha.org"),
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Alpha

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb
:END:
Body
"#,
        )
        .unwrap();
        let config = ResolvedConfig::for_test_db(dir.path());
        let outputs = execute(
            &config,
            &ValidateOptions {
                targets: vec!["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string()],
            },
        )
        .unwrap();

        assert!(outputs[0].healthy, "issues: {:?}", outputs[0].issues);
    }
}
