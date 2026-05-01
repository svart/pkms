use crate::config::Config;
use crate::discovery::discover_files;
use crate::graph::{FileScanResult, Graph};
use crate::parser::parse_note;
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct QueryOutput {
    pub query: String,
    pub total_results: usize,
    pub results: Vec<QueryResultEntry>,
}

#[derive(Serialize)]
pub struct QueryResultEntry {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub score: f64,
    pub matches: Vec<String>,
    pub content_matches: Vec<ContextLine>,
}

#[derive(Serialize)]
pub struct ContextLine {
    pub line: usize,
    pub text: String,
}

pub fn run(
    config: &Config,
    json: bool,
    ndjson: bool,
    no_header: bool,
    count_only: bool,
    verbose: bool,
    terms: &str,
    tag_filter: Option<&str>,
    limit: Option<usize>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();

    let files = discover_files(&db_root, &ignore)?;

    if verbose {
        eprintln!("Found {} .org files, parsing...", files.len());
    }

    let results: Vec<FileScanResult> = files
        .into_par_iter()
        .map(|entry| {
            let path = entry.path.clone();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let parsed = parse_note(&content);
                    FileScanResult {
                        path,
                        parsed,
                        parse_error: None,
                    }
                }
                Err(e) => FileScanResult {
                    path,
                    parsed: crate::parser::ParsedNote {
                        uuid: None,
                        title: None,
                        filetags: vec![],
                        roam_aliases: vec![],
                        roam_refs: vec![],
                        outgoing: vec![],
                        headings: vec![],
                        content_hash: String::new(),
                    },
                    parse_error: Some(format!("IO error: {}", e)),
                },
            }
        })
        .collect();

    let graph = Graph::build(results);

    let title_results = graph.search(terms);
    let content_results = graph.search_content(terms);

    // Build map of content matches
    let mut content_map: std::collections::HashMap<String, Vec<ContextLine>> =
        std::collections::HashMap::new();
    for (uuid, _title, lines) in &content_results {
        let ctx: Vec<ContextLine> = lines
            .iter()
            .map(|l| {
                let (line_str, text) = l.split_once(": ").unwrap_or(("0", l));
                ContextLine {
                    line: line_str.parse().unwrap_or(0),
                    text: text.to_string(),
                }
            })
            .collect();
        content_map.insert(uuid.clone(), ctx);
    }

    let mut combined: Vec<QueryResultEntry> = title_results
        .into_iter()
        .map(|(node, score, matches)| {
            let uuid = node.uuid.clone();
            let cm = content_map.remove(&uuid).unwrap_or_default();
            QueryResultEntry {
                uuid,
                title: node.title.clone(),
                path: node.path.to_string_lossy().to_string(),
                filetags: node.filetags.clone(),
                score,
                matches,
                content_matches: cm,
            }
        })
        .collect();

    // Add content-only matches
    for (uuid, title, lines) in content_results {
        if combined.iter().any(|r| r.uuid == uuid) {
            continue;
        }
        let ctx: Vec<ContextLine> = lines
            .iter()
            .map(|l| {
                let (line_str, text) = l.split_once(": ").unwrap_or(("0", l));
                ContextLine {
                    line: line_str.parse().unwrap_or(0),
                    text: text.to_string(),
                }
            })
            .collect();
        combined.push(QueryResultEntry {
            uuid,
            title,
            path: String::new(),
            filetags: vec![],
            score: 1.0,
            matches: vec!["content match".to_string()],
            content_matches: ctx,
        });
    }

    // Sort by score descending
    combined.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Filter by tag
    if let Some(tag) = tag_filter {
        combined.retain(|r| r.filetags.iter().any(|t| t == tag));
    }

    // Apply limit
    if let Some(limit) = limit {
        combined.truncate(limit);
    }

    if count_only {
        if json {
            println!("{}", serde_json::json!({"count": combined.len()}));
        } else {
            println!("{}", combined.len());
        }
        return Ok(());
    }

    if json {
        if ndjson {
            for r in &combined {
                println!("{}", serde_json::to_string(r)?);
            }
        } else {
            let output = QueryOutput {
                query: terms.to_string(),
                total_results: combined.len(),
                results: combined,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    } else {
        if !no_header {
            println!("Query: {}", terms);
            println!("Results: {}", combined.len());
            println!();
        }
        for (i, r) in combined.iter().enumerate() {
            println!(
                "{:3}. {}  (score: {:.1})",
                i + 1,
                r.title,
                r.score
            );
            println!("       UUID: {}", r.uuid);
            if !r.matches.is_empty() {
                println!("       Matches: {}", r.matches.join(", "));
            }
            if !r.content_matches.is_empty() {
                for cm in &r.content_matches[..std::cmp::min(3, r.content_matches.len())] {
                    println!("       > {}", cm.text);
                }
                if r.content_matches.len() > 3 {
                    println!("       ... and {} more", r.content_matches.len() - 3);
                }
            }
        }
    }

    Ok(())
}
