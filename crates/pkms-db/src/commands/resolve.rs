use anyhow::Result;
use pkms_org::OrgConfig;
use pkms_org::discovery;
use pkms_org::parser::{ParsedNoteSummary, parse_note_summary};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Write;
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
    require_todos: bool,
) -> Vec<ResolvedNote> {
    let Ok(files) = discovery::walk_org_files(root, ignore_patterns) else {
        return Vec::new();
    };

    let do_full_scan = uuid_query.is_some() || require_todos;
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
            let matched_heading = uuid_query.and_then(|uq| {
                let uq = uq.to_lowercase();
                summary
                    .uuids
                    .iter()
                    .skip(1)
                    .find(|u| u.to_lowercase().contains(&uq))
                    .cloned()
            });
            notes.push(resolved_note_from_parsed(
                path,
                &summary,
                primary_uuid.to_string(),
                matched_heading.map(|uuid| uuid.to_string()),
            ));
        } else {
            let header: Vec<&str> = content.lines().take(100).collect();
            let header_str = header.join("\n");
            let summary = parse_note_summary(&header_str);
            let Some(uuid) = summary.uuids.first().cloned() else {
                continue;
            };
            notes.push(resolved_note_from_parsed(
                path,
                &summary,
                uuid.to_string(),
                None,
            ));
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

pub struct ResolveCommandOutput {
    pub output: ResolveOutput,
    pub fields: Option<HashSet<String>>,
}

pub fn execute(config: &OrgConfig, opts: &ResolveOptions) -> Result<ResolveCommandOutput> {
    let db_root = config.db_root.as_path();
    let ignore = config.ignore_patterns.as_slice();
    let notes = scan_files(db_root, ignore, opts.uuid.as_deref(), opts.todos);

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

    Ok(ResolveCommandOutput {
        output: ResolveOutput {
            query: query_str,
            total,
            showed,
            results: shown,
        },
        fields: field_set,
    })
}

pub fn render_text(output: &ResolveOutput, field_set: Option<&HashSet<String>>) -> String {
    let mut text = String::new();

    if output.total == output.results.len() {
        let _ = writeln!(text, "Total: {}", output.results.len());
    } else {
        let _ = writeln!(
            text,
            "Total: {}, showed: {}",
            output.total,
            output.results.len()
        );
    }
    for note in &output.results {
        if let Some(fs) = field_set {
            if fs.contains("title") {
                let _ = writeln!(text, "  {}", note.title);
            }
            if fs.contains("uuid") {
                let _ = writeln!(text, "         UUID: {}", note.uuid);
            }
            if fs.contains("path") {
                let _ = writeln!(text, "         Path: {}", note.path);
            }
            if fs.contains("tags") && !note.filetags.is_empty() {
                let _ = writeln!(text, "         Tags: {}", note.filetags.join(", "));
            }
            if fs.contains("categories") && !note.categories.is_empty() {
                let _ = writeln!(text, "         Cats: {}", note.categories.join(", "));
            }
            if fs.contains("aliases") && !note.aliases.is_empty() {
                let _ = writeln!(text, "         Aliases: {}", note.aliases.join(", "));
            }
        } else {
            let _ = writeln!(text, "  {}", note.title);
            let _ = writeln!(text, "         UUID: {}", note.uuid);
            if !note.filetags.is_empty() {
                let _ = writeln!(text, "         Tags: {}", note.filetags.join(", "));
            }
            if !note.categories.is_empty() {
                let _ = writeln!(text, "         Cats: {}", note.categories.join(", "));
            }
        }
    }

    text
}

pub fn filter_fields(
    value: &serde_json::Value,
    fields: Option<&HashSet<String>>,
) -> serde_json::Value {
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

    fn resolved_note() -> ResolvedNote {
        ResolvedNote {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: "Note A".to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            categories: vec!["cat".to_string()],
            aliases: vec!["Alias A".to_string()],
            matched_heading_uuid: None,
            has_todos: false,
        }
    }

    #[test]
    fn renders_default_text_from_typed_output() {
        let output = ResolveOutput {
            query: "Note".to_string(),
            total: 1,
            showed: None,
            results: vec![resolved_note()],
        };

        let text = render_text(&output, None);

        assert!(text.contains("Total: 1"));
        assert!(text.contains("  Note A"));
        assert!(text.contains("         UUID: 11111111-1111-4111-8111-111111111111"));
        assert!(text.contains("         Tags: tag"));
        assert!(text.contains("         Cats: cat"));
    }

    #[test]
    fn renders_selected_text_fields_and_limit_metadata() {
        let output = ResolveOutput {
            query: "Note".to_string(),
            total: 3,
            showed: Some(1),
            results: vec![resolved_note()],
        };
        let fields = HashSet::from(["title".to_string(), "path".to_string()]);

        let text = render_text(&output, Some(&fields));

        assert!(text.contains("Total: 3, showed: 1"));
        assert!(text.contains("  Note A"));
        assert!(text.contains("         Path: /notes/a.org"));
        assert!(!text.contains("UUID:"));
        assert!(!text.contains("Tags:"));
    }

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
    fn resolve_todos_finds_tasks_after_header_scan_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut content = String::from(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Alpha

"#,
        );
        for i in 0..110 {
            let _ = writeln!(content, "Line {i}");
        }
        content.push_str("* TODO Late task\n");
        std::fs::write(dir.path().join("alpha.org"), content).unwrap();
        let config = OrgConfig {
            db_root: dir.path().to_path_buf(),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: Vec::new(),
        };
        let output = execute(
            &config,
            &ResolveOptions {
                uuid: None,
                title: Some("Alpha".to_string()),
                tags: None,
                limit: None,
                fields: None,
                todos: true,
            },
        )
        .unwrap();

        assert_eq!(output.output.total, 1);
        assert!(output.output.results[0].has_todos);
    }
}
