use crate::config::Config;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
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

static UUID_RE: LazyLock<Regex> =
    LazyLock::new(|| {
        Regex::new(r":ID:\s+([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})")
            .unwrap()
    });

static TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+title:\s*(.*)$").unwrap());

static FILETAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+filetags:\s*(.+)$").unwrap());

static ALIASES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":ROAM_ALIASES:\s+(.*)").unwrap());

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

fn scan_files(root: &Path, ignore_patterns: &[String]) -> Vec<ResolvedNote> {
    let compiled_patterns: Vec<glob::Pattern> = ignore_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut notes = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_hidden(e) && !compiled_patterns.iter().any(|p| p.matches(&e.file_name().to_string_lossy())))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.path().extension().map_or(true, |e| e != "org") {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(entry.path()) else { continue };
        // Only read first ~30 lines for header metadata
        let header: Vec<&str> = content.lines().take(30).collect();
        let header_str = header.join("\n");

        let uuid = UUID_RE.captures(&header_str).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
        let Some(uuid) = uuid else { continue };

        let title = TITLE_RE.captures(&header_str).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_string()).unwrap_or_default();

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
            .captures(&header_str)
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

pub fn run(
    config: &Config,
    json: bool,
    _verbose: bool,
    target: Option<&str>,
    tags: Option<&str>,
    search: Option<&str>,
    limit: Option<usize>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();
    let notes = scan_files(&db_root, &ignore);

    let limit = limit.unwrap_or(30);
    let query = target.unwrap_or("");
    let query_lower = query.to_lowercase();
    let search_lower = search.map(|s| s.to_lowercase());

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
                if n.aliases.iter().any(|a| a.to_lowercase().contains(&query_lower)) {
                    return true;
                }
            }
            if let Some(ref s) = search_lower {
                if n.title.to_lowercase().contains(s) || n.aliases.iter().any(|a| a.to_lowercase().contains(s)) {
                    return true;
                }
            }
            if let Some(t) = tags {
                let wanted: Vec<&str> = t.split(',').map(|t| t.trim()).collect();
                if wanted.iter().any(|w| n.filetags.contains(&w.to_string())) {
                    return true;
                }
            }
            false
        })
        .collect();

    // Sort: exact UUID match first, then title match, then prefix match
    results.sort_by(|a, b| {
        let a_score = score_match(&a, query, &query_lower);
        let b_score = score_match(&b, query, &query_lower);
        b_score.cmp(&a_score)
    });

    results.truncate(limit);

    if json {
        let output = ResolveOutput {
            query: query.to_string(),
            total: results.len(),
            results,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if !query.is_empty() {
            println!("Resolved: \"{}\"", query);
        }
        println!("Total: {}", results.len());
        for note in &results {
            let short = if note.uuid.len() > 8 { &note.uuid[..8] } else { &note.uuid };
            println!("  {} ({})", note.title, short);
            println!("         UUID: {}", note.uuid);
            if !note.filetags.is_empty() {
                println!("         Tags: {}", note.filetags.join(", "));
            }
        }
    }

    Ok(())
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
