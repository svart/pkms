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

pub struct ResolveOptions<'a> {
    pub uuid: Option<&'a str>,
    pub title: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub limit: Option<usize>,
    pub fields: Option<&'a str>,
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    opts: &ResolveOptions,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();
    let notes = scan_files(&db_root, &ignore);

    let limit = opts.limit.unwrap_or(30);
    let uuid_query = opts.uuid.map(str::to_lowercase);
    let title_query = opts.title.map(str::to_lowercase);

    let mut results: Vec<ResolvedNote> = notes
        .into_iter()
        .filter(|n| {
            if let Some(ref uq) = uuid_query
                && !n.uuid.to_lowercase().contains(uq)
            {
                return false;
            }
            if let Some(ref tq) = title_query
                && !n.title.to_lowercase().contains(tq)
            {
                return false;
            }
            if let Some(t) = opts.tags {
                let wanted: Vec<&str> = t.split(',').map(str::trim).collect();
                if !wanted
                    .iter()
                    .any(|w| n.filetags.iter().any(|ft| ft.contains(w)))
                {
                    return false;
                }
            }
            true
        })
        .collect();

    results.sort_by(|a, b| {
        let a_score = score_note(a, &uuid_query, &title_query);
        let b_score = score_note(b, &uuid_query, &title_query);
        b_score.cmp(&a_score)
    });

    results.truncate(limit);

    let field_set: Option<HashSet<String>> = opts
        .fields
        .map(|f| f.split(',').map(|s| s.trim().to_string()).collect());

    print_resolve_output(ctx, results, field_set.as_ref())?;

    Ok(())
}

fn print_resolve_output(
    ctx: &OutputContext,
    results: Vec<ResolvedNote>,
    field_set: Option<&HashSet<String>>,
) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            println!("Total: {}", results.len());
            for note in &results {
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
                    if fs.contains("aliases") && !note.aliases.is_empty() {
                        println!("         Aliases: {}", note.aliases.join(", "));
                    }
                } else {
                    println!("  {}", note.title);
                    println!("         UUID: {}", note.uuid);
                    if !note.filetags.is_empty() {
                        println!("         Tags: {}", note.filetags.join(", "));
                    }
                }
            }
        }
        OutputFormat::Json => {
            let output = ResolveOutput {
                query: String::new(),
                total: results.len(),
                results,
            };
            ctx.print_json(&output)?;
        }
        OutputFormat::Ndjson => {
            for note in &results {
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

    #[test]
    fn test_score_note_exact_uuid() {
        let note = ResolvedNote {
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "a.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let uuid_q = Some("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string());
        assert_eq!(score_note(&note, &uuid_q, &None), 100);
    }

    #[test]
    fn test_score_note_uuid_prefix() {
        let note = ResolvedNote {
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "a.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let uuid_q = Some("aaaaaaaa".to_string());
        assert_eq!(score_note(&note, &uuid_q, &None), 60);
    }

    #[test]
    fn test_score_note_uuid_contains() {
        let note = ResolvedNote {
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "a.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let uuid_q = Some("aaaa".to_string());
        assert_eq!(score_note(&note, &uuid_q, &None), 60); // starts_with matches
    }

    #[test]
    fn test_score_note_exact_title() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Exact Title".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let title_q = Some("exact title".to_string());
        assert_eq!(score_note(&note, &None, &title_q), 80);
    }

    #[test]
    fn test_score_note_title_contains() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Exact Title".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let title_q = Some("title".to_string());
        assert_eq!(score_note(&note, &None, &title_q), 25); // contains
    }

    #[test]
    fn test_score_note_title_starts_with() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Exact Title".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let title_q = Some("exact".to_string());
        assert_eq!(score_note(&note, &None, &title_q), 50);
    }

    #[test]
    fn test_score_note_both_queries() {
        let note = ResolvedNote {
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "a.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        let uuid_q = Some("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string());
        let title_q = Some("note".to_string());
        assert_eq!(score_note(&note, &uuid_q, &title_q), 100 + 50);
    }

    #[test]
    fn test_score_note_no_queries() {
        let note = ResolvedNote {
            uuid: "x".to_string(),
            title: "Note".to_string(),
            path: "x.org".to_string(),
            filetags: vec![],
            aliases: vec![],
        };
        assert_eq!(score_note(&note, &None, &None), 0);
    }
}

fn score_note(
    note: &ResolvedNote,
    uuid_query: &Option<String>,
    title_query: &Option<String>,
) -> usize {
    let mut score = 0;
    if let Some(uq) = uuid_query {
        let uuid_lower = note.uuid.to_lowercase();
        if uuid_lower == *uq {
            score += 100;
        } else if uuid_lower.starts_with(uq) {
            score += 60;
        } else if uuid_lower.contains(uq.as_str()) {
            score += 30;
        }
    }
    if let Some(tq) = title_query {
        let title_lower = note.title.to_lowercase();
        if title_lower == *tq {
            score += 80;
        } else if title_lower.starts_with(tq) {
            score += 50;
        } else if title_lower.contains(tq.as_str()) {
            score += 25;
        }
    }
    score
}
