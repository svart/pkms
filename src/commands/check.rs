use crate::config::Config;
use crate::discovery::discover_files;
use crate::graph::{FileScanResult, Graph};
use crate::parser::parse_note;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct CheckOutput {
    pub db_root: String,
    pub stats: crate::graph::GraphStats,
    pub broken_links: Vec<BrokenLinkEntry>,
    pub failed_files: Vec<FailedFileEntry>,
}

#[derive(Serialize)]
pub struct BrokenLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
}

#[derive(Serialize)]
pub struct FailedFileEntry {
    pub path: String,
    pub error: String,
}

pub fn run(config: &Config, json: bool, verbose: bool, db_cli: Option<&std::path::Path>) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();

    if verbose {
        eprintln!("Scanning: {}", db_root.display());
    }

    let files = discover_files(&db_root, &ignore)?;

    if verbose {
        eprintln!("Found {} .org files, parsing...", files.len());
    }

    let results: Vec<FileScanResult> = files
        .into_iter()
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
    let stats = graph.stats();

    if json {
        let broken = graph
            .broken_links
            .iter()
            .map(|(src, tgt)| BrokenLinkEntry {
                source_uuid: src.clone(),
                source_title: graph
                    .nodes
                    .get(src)
                    .map(|n| n.title.clone())
                    .unwrap_or_default(),
                target_uuid: tgt.clone(),
            })
            .collect();

        let failed = graph
            .parse_errors
            .iter()
            .map(|(path, err)| FailedFileEntry {
                path: path.to_string_lossy().to_string(),
                error: err.clone(),
            })
            .collect();

        let output = CheckOutput {
            db_root: db_root.to_string_lossy().to_string(),
            stats,
            broken_links: broken,
            failed_files: failed,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Database: {}", db_root.display());
        println!("  Notes:          {}", stats.total_notes);
        println!("  Links:          {} (internal: {}, file: {}, url: {})",
            stats.total_links,
            stats.total_internal_links,
            stats.total_file_links,
            stats.total_url_links,
        );
        println!("  Orphans:        {}", stats.orphan_notes);
        println!("  Broken links:   {}", stats.broken_link_count);
        println!("  Parse errors:   {}", stats.parse_error_count);
        println!("  Skipped files:  {}", stats.skipped_count);

        if !graph.broken_links.is_empty() {
            println!();
            println!("Broken links:");
            for (src, tgt) in &graph.broken_links {
                let title = graph
                    .nodes
                    .get(src)
                    .map(|n| n.title.as_str())
                    .unwrap_or("?");
                println!("  {} -> {}", title, tgt);
            }
        }

        if verbose && !graph.parse_errors.is_empty() {
            println!();
            println!("Parse errors:");
            for (path, err) in &graph.parse_errors {
                println!("  {}: {}", path.display(), err);
            }
        }

        if verbose && !graph.skipped_files.is_empty() {
            println!();
            println!("Skipped files (no :ID:):");
            for path in &graph.skipped_files {
                println!("  {}", path.display());
            }
        }
    }

    Ok(())
}
