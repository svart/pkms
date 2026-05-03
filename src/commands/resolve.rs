use crate::cli::OutputFormat;
use crate::config::Config;
use crate::discovery;
use crate::output::OutputContext;
use crate::parser::{FILETAGS_RE, TITLE_RE};
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Serialize)]
pub struct ResolveOutput {
    pub query: String,
    pub total: usize,
    pub results: Vec<ResolvedNote>,
}

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":ID:\s+([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})").unwrap()
});

static ALIASES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":ROAM_ALIASES:\s+(.*)").unwrap());

fn scan_files(root: &Path, ignore_patterns: &[String]) -> Vec<ResolvedNote> {
    let compiled_patterns: Vec<glob::Pattern> = ignore_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let root_clone = root.to_path_buf();
    let mut notes = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| {
            if e.path() == root_clone {
                return true;
            }
            !discovery::is_ignored(e, &compiled_patterns)
        })
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "org") {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let header: Vec<&str> = content.lines().take(100).collect();
        let header_str = header.join("\n");

        let uuid = UUID_RE
            .captures_iter(&header_str)
            .last()
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        let Some(uuid) = uuid else { continue };

        let title = TITLE_RE
            .captures(&header_str)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let filetags = FILETAGS_RE
            .captures(&header_str)
            .map(|c| {
                c.get(1)
                    .map_or("", |m| m.as_str())
                    .split(':')
                    .filter(|t| !t.is_empty())
                    .map(|t| t.trim().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let aliases = ALIASES_RE
            .captures_iter(&header_str)
            .last()
            .map(|c| {
                c.get(1)
                    .map_or("", |m| m.as_str())
                    .split_whitespace()
                    .map(|s| s.trim_matches('"').to_string())
                    .collect()
            })
            .unwrap_or_default();

        notes.push(ResolvedNote {
            uuid,
            title,
            path: entry.path().to_string_lossy().to_string(),
            filetags,
            aliases,
        });
    }

    notes
}

#[allow(clippy::too_many_lines)]
pub fn run(
    config: &Config,
    ctx: &OutputContext,
    target: Option<&str>,
    tags: Option<&str>,
    search: Option<&str>,
    limit: Option<usize>,
    fields: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();
    let notes = scan_files(&db_root, &ignore);

    let limit = limit.unwrap_or(30);
    let query = target.unwrap_or("");
    let query_lower = query.to_lowercase();
    let search_lower = search.map(str::to_lowercase);

    let mut results: Vec<ResolvedNote> = notes
        .into_iter()
        .filter(|n| {
            if query.is_empty() && search.is_none() && tags.is_none() {
                return true;
            }
            if !query.is_empty() {
                if n.uuid.to_lowercase().contains(&query_lower) {
                    return true;
                }
                if n.title.to_lowercase().contains(&query_lower) {
                    return true;
                }
                if n.aliases
                    .iter()
                    .any(|a| a.to_lowercase().contains(&query_lower))
                {
                    return true;
                }
            }
            if let Some(ref s) = search_lower
                && (n.title.to_lowercase().contains(s)
                    || n.aliases.iter().any(|a| a.to_lowercase().contains(s)))
            {
                return true;
            }
            if let Some(t) = tags {
                let wanted: Vec<&str> = t.split(',').map(str::trim).collect();
                if wanted.iter().any(|w| n.filetags.contains(&w.to_string())) {
                    return true;
                }
            }
            false
        })
        .collect();

    results.sort_by(|a, b| {
        let a_score = score_match(a, query, &query_lower);
        let b_score = score_match(b, query, &query_lower);
        b_score.cmp(&a_score)
    });

    results.truncate(limit);

    if ctx.count_only && ctx.format != OutputFormat::Ndjson {
        ctx.print_count(results.len());
        return Ok(());
    }

    let field_set: Option<HashSet<String>> =
        fields.map(|f| f.split(',').map(|s| s.trim().to_string()).collect());

    match ctx.format {
        OutputFormat::Text => {
            if !ctx.no_header {
                if !query.is_empty() {
                    println!("Resolved: \"{query}\"");
                }
                println!("Total: {}", results.len());
            }
            for note in &results {
                let short = if note.uuid.len() > 8 {
                    &note.uuid[..8]
                } else {
                    &note.uuid
                };
                if let Some(ref fs) = field_set {
                    if fs.contains("title") {
                        println!("  {} ({})", note.title, short);
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
                    if fs.contains("aliases") && !note.aliases.is_empty() {
                        println!("         Aliases: {}", note.aliases.join(", "));
                    }
                } else {
                    println!("  {} ({})", note.title, short);
                    println!("         UUID: {}", note.uuid);
                    if !note.filetags.is_empty() {
                        println!("         Tags: {}", note.filetags.join(", "));
                    }
                }
            }
        }
        OutputFormat::Json => {
            let output = ResolveOutput {
                query: query.to_string(),
                total: results.len(),
                results,
            };
            ctx.print_json(&output)?;
        }
        OutputFormat::Ndjson => {
            for note in &results {
                let v = filter_fields(&serde_json::to_value(note)?, &field_set);
                println!("{}", serde_json::to_string(&v)?);
            }
        }
    }

    Ok(())
}

fn filter_fields(value: &serde_json::Value, fields: &Option<HashSet<String>>) -> serde_json::Value {
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
        let result = filter_fields(&v, &None);
        assert_eq!(result, v);
    }

    #[test]
    fn test_filter_fields_subset() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "path": "/a.org", "filetags": ["tag1"]});
        let fields = Some(HashSet::from(["uuid".to_string(), "title".to_string()]));
        let result = filter_fields(&v, &fields);
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
        let result = filter_fields(&v, &fields);
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("uuid"));
        assert!(obj.contains_key("score"), "score should be preserved");
    }

    #[test]
    fn test_filter_fields_non_object() {
        let v = serde_json::json!("just a string");
        let fields = Some(HashSet::from(["key".to_string()]));
        let result = filter_fields(&v, &fields);
        assert_eq!(result, v);
    }

    #[test]
    fn test_score_match_exact_uuid() {
        let note = ResolvedNote {
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "a.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        assert_eq!(
            score_match(
                &note,
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                &"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_lowercase()
            ),
            100
        );
    }

    #[test]
    fn test_score_match_exact_title() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Exact Title".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        assert_eq!(
            score_match(&note, "Exact Title", &"exact title".to_lowercase()),
            80
        );
    }

    #[test]
    fn test_score_match_alias() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Note".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec!["MyAlias".to_string()],
        };
        assert_eq!(score_match(&note, "MyAlias", &"myalias".to_lowercase()), 40);
    }

    #[test]
    fn test_score_match_empty_query() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Note".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        assert_eq!(score_match(&note, "", ""), 0);
    }
}

fn score_match(note: &ResolvedNote, query: &str, query_lower: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    if note.uuid == query {
        return 100;
    }
    if note.uuid.to_lowercase() == query_lower {
        return 90;
    }
    if note.title == query {
        return 80;
    }
    if note.title.to_lowercase() == query_lower {
        return 70;
    }
    if note.uuid.to_lowercase().starts_with(query_lower) {
        return 60;
    }
    if note.title.to_lowercase().contains(query_lower) {
        return 50;
    }
    if note.aliases.iter().any(|a| a.to_lowercase() == query_lower) {
        return 40;
    }
    0
}
