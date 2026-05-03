use crate::config::Config;
use crate::graph::{DuplicateInfo, Graph, GraphStats};
use crate::output::OutputContext;
use crate::parser::Link;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::process::ExitCode;

#[derive(Serialize)]
pub struct CheckOutput {
    pub db_root: String,
    pub stats: GraphStats,
    pub duplicates: DuplicateInfo,
    pub broken_links: Vec<BrokenLinkEntry>,
    pub broken_file_links: Vec<BrokenFileLinkEntry>,
    pub broken_attachment_links: Vec<BrokenAttachmentLinkEntry>,
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
pub struct BrokenFileLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_path: String,
}

#[derive(Serialize)]
pub struct BrokenAttachmentLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_path: String,
}

#[derive(Serialize)]
pub struct FailedFileEntry {
    pub path: String,
    pub error: String,
}

fn link_target_exists(target: &str, db_root: &Path) -> bool {
    let path = Path::new(target);
    if path.is_absolute() {
        path.exists()
    } else {
        db_root.join(path).exists()
    }
}

fn print_check_json(
    ctx: &OutputContext,
    graph: &Graph,
    db_root: &Path,
    broken_file: Vec<BrokenFileLinkEntry>,
    broken_attachment: Vec<BrokenAttachmentLinkEntry>,
    healthy: bool,
) -> Result<()> {
    let stats = graph.stats();
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
        duplicates: graph.duplicates.clone(),
        broken_links: broken,
        broken_file_links: broken_file,
        broken_attachment_links: broken_attachment,
        failed_files: failed,
        healthy,
    };
    ctx.print_json(&output)
}

fn print_check_text(
    graph: &Graph,
    db_root: &Path,
    file_links: bool,
    attachment_links: bool,
    broken_file: &[BrokenFileLinkEntry],
    broken_attachment: &[BrokenAttachmentLinkEntry],
    verbose: bool,
) {
    let stats = graph.stats();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty();
    let broken_file_links_count = broken_file.len();
    let broken_attachment_links_count = broken_attachment.len();
    println!("Database: {}", db_root.display());
    println!("  Notes:          {}", stats.total_notes);
    println!(
        "  Links:          {} (internal: {}, file: {}, url: {})",
        stats.total_links,
        stats.total_internal_links,
        stats.total_file_links,
        stats.total_url_links,
    );
    println!("  Orphans:        {}", stats.orphan_notes);
    println!("  Broken links:   {}", stats.broken_link_count);
    if file_links {
        println!("  Broken files:   {broken_file_links_count}");
    }
    if attachment_links {
        println!("  Broken attach:  {broken_attachment_links_count}");
    }
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
            println!("  {p}");
        }
    }

    if !graph.broken_links.is_empty() {
        println!();
        println!("Broken links:");
        for (src, tgt) in &graph.broken_links {
            let title = graph.nodes.get(src).map_or("?", |n| n.title.as_str());
            println!("  {title} -> {tgt}");
        }
    }

    if !broken_file.is_empty() {
        println!();
        println!("Broken file links:");
        for entry in broken_file {
            println!("  {} -> {}", entry.source_title, entry.target_path);
        }
    }

    if !broken_attachment.is_empty() {
        println!();
        println!("Broken attachment links:");
        for entry in broken_attachment {
            println!("  {} -> {}", entry.source_title, entry.target_path);
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

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    verbose: bool,
    db_cli: Option<&std::path::Path>,
    file_links: bool,
    attachment_links: bool,
) -> Result<ExitCode> {
    let graph = Graph::load(config, db_cli, verbose)?;
    let stats = graph.stats();
    let db_root = config.resolve_db_root(db_cli)?;

    let mut broken_file = Vec::new();
    let mut broken_attachment = Vec::new();

    if file_links {
        for node in graph.nodes.values() {
            for link in &node.outgoing {
                if let Link::File(target) = link
                    && !link_target_exists(target, &db_root)
                {
                    broken_file.push(BrokenFileLinkEntry {
                        source_uuid: node.uuid.clone(),
                        source_title: node.title.clone(),
                        target_path: target.clone(),
                    });
                }
            }
        }
    }

    if attachment_links {
        for node in graph.nodes.values() {
            for link in &node.outgoing {
                if let Link::Attachment(target) = link
                    && !link_target_exists(target, &db_root)
                {
                    broken_attachment.push(BrokenAttachmentLinkEntry {
                        source_uuid: node.uuid.clone(),
                        source_title: node.title.clone(),
                        target_path: target.clone(),
                    });
                }
            }
        }
    }

    let broken_file_links_count = broken_file.len();
    let broken_attachment_links_count = broken_attachment.len();

    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && broken_file_links_count == 0
        && broken_attachment_links_count == 0;

    if ctx.is_json() {
        print_check_json(
            ctx,
            &graph,
            &db_root,
            broken_file,
            broken_attachment,
            healthy,
        )?;
    } else {
        print_check_text(
            &graph,
            &db_root,
            file_links,
            attachment_links,
            &broken_file,
            &broken_attachment,
            verbose,
        );
    }

    if healthy {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}
