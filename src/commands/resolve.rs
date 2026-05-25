use crate::cli::{OutputFormat, ResolveArgs};
use crate::config::ResolvedConfig;
use crate::discovery;
use crate::input;
use crate::output::OutputContext;
use crate::parser::{ParsedNoteSummary, parse_note_summary};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_heading_uuid: Option<String>,
    pub has_todos: bool,
}

#[derive(Serialize)]
pub struct ResolveOutput {
    pub query: String,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub showed: Option<usize>,
    pub results: Vec<ResolvedNote>,
}

fn scan_files(
    root: &Path,
    ignore_patterns: &[String],
    uuid_query: Option<&str>,
) -> Vec<ResolvedNote> {
    let Ok(files) = discovery::walk_org_files(root, ignore_patterns) else {
        return Vec::new();
    };

    let do_full_scan = uuid_query.is_some();
    let mut notes = Vec::new();

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        if do_full_scan {
            let summary = parse_note_summary(&content);
            let Some(primary_uuid) = summary.uuids.first().cloned() else {
                continue;
            };
            let uq = uuid_query.unwrap();
            let matched_heading = summary
                .uuids
                .iter()
                .skip(1)
                .find(|u| u.to_lowercase().contains(&uq.to_lowercase()))
                .cloned();
            notes.push(resolved_note_from_parsed(
                path,
                &summary,
                primary_uuid,
                matched_heading,
            ));
        } else {
            let header: Vec<&str> = content.lines().take(100).collect();
            let header_str = header.join("\n");
            let summary = parse_note_summary(&header_str);
            let Some(uuid) = summary.uuids.first().cloned() else {
                continue;
            };
            notes.push(resolved_note_from_parsed(path, &summary, uuid, None));
        }
    }

    notes
}

fn resolved_note_from_parsed(
    path: &std::path::Path,
    summary: &ParsedNoteSummary,
    uuid: String,
    matched_heading: Option<String>,
) -> ResolvedNote {
    ResolvedNote {
        uuid,
        title: summary.title.clone(),
        path: path.display().to_string(),
        filetags: summary.filetags.clone(),
        categories: summary.categories.clone(),
        aliases: summary.aliases.clone(),
        matched_heading_uuid: matched_heading,
        has_todos: summary.has_todos,
    }
}

pub struct ResolveOptions {
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub fields: Option<Vec<String>>,
    pub todos: bool,
}

impl From<&ResolveArgs> for ResolveOptions {
    fn from(args: &ResolveArgs) -> Self {
        ResolveOptions {
            uuid: args.uuid.clone(),
            title: args.title.clone(),
            tags: input::comma_list(args.tags.as_deref()),
            limit: args.limit,
            fields: input::comma_list(args.fields.as_deref()),
            todos: args.todos,
        }
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &ResolveOptions) -> Result<()> {
    let db_root = config.resolved_db_root();
    let ignore = config.resolve_ignore_patterns();
    let notes = scan_files(db_root, &ignore, opts.uuid.as_deref());

    let title_query = opts.title.as_ref().map(|s| s.to_lowercase());

    let mut all: Vec<ResolvedNote> = notes
        .into_iter()
        .filter(|n| {
            if let Some(ref uq) = opts.uuid
                && !n.uuid.to_lowercase().contains(&uq.to_lowercase())
                && n.matched_heading_uuid
                    .as_ref()
                    .is_none_or(|h| !h.to_lowercase().contains(&uq.to_lowercase()))
            {
                return false;
            }
            if let Some(ref tq) = title_query {
                let words: Vec<&str> = tq.split_whitespace().collect();
                let title_match = words.iter().all(|w| n.title.to_lowercase().contains(w));
                let alias_match = n
                    .aliases
                    .iter()
                    .any(|a| words.iter().all(|w| a.to_lowercase().contains(w)));
                if !title_match && !alias_match {
                    return false;
                }
            }
            if let Some(ref tags) = opts.tags {
                let matches_tag = tags
                    .iter()
                    .any(|w| n.filetags.iter().any(|ft| ft.contains(w)));
                let matches_category = tags
                    .iter()
                    .any(|w| n.categories.iter().any(|c| c.contains(w)));
                if !matches_tag && !matches_category {
                    return false;
                }
            }
            if opts.todos && !n.has_todos {
                return false;
            }
            true
        })
        .collect();

    all.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.uuid.cmp(&b.uuid)));

    let total = all.len();
    let (shown, showed) = if let Some(l) = opts.limit {
        let shown: Vec<_> = all.into_iter().take(l).collect();
        let showed = shown.len();
        (shown, Some(showed))
    } else {
        (all, None)
    };

    let field_set: Option<HashSet<String>> =
        opts.fields.as_ref().map(|f| f.iter().cloned().collect());

    let query_str = opts
        .title
        .as_deref()
        .or(opts.uuid.as_deref())
        .or_else(|| opts.tags.as_ref().map(|_| "tags"))
        .unwrap_or_default()
        .to_string();

    print_resolve_output(ctx, &shown, total, showed, field_set.as_ref(), &query_str)?;

    Ok(())
}

fn print_resolve_output(
    ctx: &OutputContext,
    results: &[ResolvedNote],
    total: usize,
    showed: Option<usize>,
    field_set: Option<&HashSet<String>>,
    query_str: &str,
) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            if total == results.len() {
                println!("Total: {}", results.len());
            } else {
                println!("Total: {}, showed: {}", total, results.len());
            }
            for note in results {
                if let Some(fs) = field_set {
                    if fs.contains("title") {
                        println!("  {}", note.title);
                    }
                    if fs.contains("uuid") {
                        println!("         UUID: {}", note.uuid);
                    }
                    if fs.contains("path") {
                        println!("         Path: {}", note.path);
                    }
                    if fs.contains("tags") && !note.filetags.is_empty() {
                        println!("         Tags: {}", note.filetags.join(", "));
                    }
                    if fs.contains("categories") && !note.categories.is_empty() {
                        println!("         Cats: {}", note.categories.join(", "));
                    }
                    if fs.contains("aliases") && !note.aliases.is_empty() {
                        println!("         Aliases: {}", note.aliases.join(", "));
                    }
                } else {
                    println!("  {}", note.title);
                    println!("         UUID: {}", note.uuid);
                    if !note.filetags.is_empty() {
                        println!("         Tags: {}", note.filetags.join(", "));
                    }
                    if !note.categories.is_empty() {
                        println!("         Cats: {}", note.categories.join(", "));
                    }
                }
            }
        }
        OutputFormat::Json => {
            let output = ResolveOutput {
                query: query_str.to_string(),
                total,
                showed,
                results: results.to_vec(),
            };
            ctx.print_json(&output)?;
        }
        OutputFormat::Ndjson => {
            for note in results {
                let v = filter_fields(&serde_json::to_value(note)?, field_set);
                println!("{}", serde_json::to_string(&v)?);
            }
        }
    }

    Ok(())
}

fn filter_fields(value: &serde_json::Value, fields: Option<&HashSet<String>>) -> serde_json::Value {
    let Some(fs) = fields else {
        return value.clone();
    };
    match value {
        serde_json::Value::Object(map) => {
            let mut filtered = serde_json::Map::new();
            for (k, v) in map {
                if fs.contains(k) || k == "score" || k == "matches" || k == "content_matches" {
                    filtered.insert(k.clone(), v.clone());
                }
            }
            serde_json::Value::Object(filtered)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_fields_none() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "path": "/a.org", "filetags": ["tag1"]});
        let result = filter_fields(&v, None::<&HashSet<String>>);
        assert_eq!(result, v);
    }

    #[test]
    fn test_filter_fields_subset() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "path": "/a.org", "filetags": ["tag1"]});
        let fields = Some(HashSet::from(["uuid".to_string(), "title".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("uuid"));
        assert!(obj.contains_key("title"));
        assert!(!obj.contains_key("path"));
        assert!(!obj.contains_key("filetags"));
    }

    #[test]
    fn test_filter_fields_preserves_score() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "score": 42.0, "filetags": []});
        let fields = Some(HashSet::from(["uuid".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("uuid"));
        assert!(obj.contains_key("score"), "score should be preserved");
    }

    #[test]
    fn test_filter_fields_non_object() {
        let v = serde_json::json!("just a string");
        let fields = Some(HashSet::from(["key".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        assert_eq!(result, v);
    }
}
