use crate::config::Config;
use crate::graph::{DuplicateInfo, Graph, GraphStats};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct CheckOutput {
    pub db_root: String,
    pub stats: GraphStats,
    pub duplicates: DuplicateInfo,
    pub broken_links: Vec<BrokenLinkEntry>,
    pub failed_files: Vec<FailedFileEntry>,
    pub healthy: bool,
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

pub fn run(config: &Config, json: bool, verbose: bool, db_cli: Option<&std::path::Path>) -> Result<bool> {
    let graph = Graph::load(config, db_cli, verbose)?;
    let stats = graph.stats();

    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0;

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
            db_root: graph
                .nodes
                .values()
                .next()
                .and_then(|n| n.path.parent())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            stats,
            duplicates: graph.duplicates,
            broken_links: broken,
            failed_files: failed,
            healthy,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Database: {}", graph.nodes.values().next().and_then(|n| n.path.parent()).map(|p| p.display().to_string()).unwrap_or_default());
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
        println!("  Dup UUIDs:      {}", stats.duplicate_uuid_count);
        println!("  Dup titles:     {}", stats.duplicate_title_count);
        println!("  Missing titles: {}", stats.missing_title_count);

        if !graph.duplicates.duplicate_uuids.is_empty() {
            println!();
            println!("Duplicate UUIDs:");
            for d in &graph.duplicates.duplicate_uuids {
                for p in &d.paths {
                    println!("  {} -> {}", d.value, p);
                }
            }
        }

        if !graph.duplicates.duplicate_titles.is_empty() {
            println!();
            println!("Duplicate titles:");
            for d in &graph.duplicates.duplicate_titles {
                for p in &d.paths {
                    println!("  \"{}\" -> {}", d.value, p);
                }
            }
        }

        if !graph.duplicates.missing_titles.is_empty() {
            println!();
            println!("Missing #+title:");
            for p in &graph.duplicates.missing_titles {
                println!("  {}", p);
            }
        }

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

        println!();
        if healthy {
            println!("Status: healthy");
        } else {
            println!("Status: issues found");
        }
    }

    Ok(healthy)
}
