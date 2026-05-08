use crate::config::Config;
use crate::graph::{DuplicateInfo, Graph, GraphStats, HeadingBacklinkEntry};
use crate::output::OutputContext;
use crate::parser::{Link, validate_filetags_format};
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
    pub filetags_issues: Vec<FiletagsIssue>,
    pub heading_backlinks: Vec<HeadingBacklinkEntry>,
    pub healthy: bool,
}

#[derive(Serialize)]
pub struct BrokenLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
}

#[derive(Clone, Serialize)]
pub struct BrokenFileLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_path: String,
}

#[derive(Clone, Serialize)]
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

#[derive(Clone, Serialize)]
pub struct FiletagsIssue {
    pub path: String,
    pub title: String,
    pub issue: String,
}

fn link_target_exists(target: &str, db_root: &Path) -> bool {
    let expanded = if target.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            target.replacen('~', &home.to_string_lossy(), 1)
        } else {
            target.to_string()
        }
    } else {
        target.to_string()
    };
    let path_str = expanded.split("::").next().unwrap_or(&expanded);
    let path = Path::new(path_str);
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        db_root.join(path)
    };
    if !full_path.exists() {
        return false;
    }
    if let Some(line_spec) = expanded.split_once("::").map(|x| x.1) {
        if line_spec.is_empty() {
            return true;
        }
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            return content.lines().any(|l| l.contains(line_spec));
        }
        return false;
    }
    true
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    db_cli: Option<&std::path::Path>,
    file_links: bool,
    attachment_links: bool,
    id_links: bool,
    filetags: bool,
) -> Result<ExitCode> {
    let graph = Graph::load(config, db_cli)?;
    let db_root = config.resolve_db_root(db_cli)?;

    let any_explicit = id_links || file_links || attachment_links || filetags;
    let show_id = id_links || !any_explicit;
    let show_file = file_links || !any_explicit;
    let show_attach = attachment_links || !any_explicit;
    let show_filetags = filetags || !any_explicit;

    let mut broken_file = Vec::new();
    let mut broken_attachment = Vec::new();

    if show_file {
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

    if show_attach {
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

    let broken_file_count = broken_file.len();
    let broken_attachment_count = broken_attachment.len();
    let broken_internal = if show_id {
        graph.stats().broken_link_count
    } else {
        0
    };

    let mut filetags_issues = Vec::new();
    if show_filetags {
        for node in graph.nodes.values() {
            if let Ok(content) = std::fs::read_to_string(&node.path) {
                for (raw, reason) in validate_filetags_format(&content) {
                    filetags_issues.push(FiletagsIssue {
                        path: node.path.to_string_lossy().to_string(),
                        title: node.title.clone(),
                        issue: format!("tag '{raw}' — {reason}"),
                    });
                }
            }
        }
    }
    let filetags_issue_count = filetags_issues.len();

    let healthy = broken_internal == 0
        && broken_file_count == 0
        && broken_attachment_count == 0
        && filetags_issue_count == 0
        && (!show_id || graph.stats().parse_error_count == 0)
        && (!show_id || graph.stats().duplicate_uuid_count == 0);

    let heading_backlinks = graph.heading_backlinks();

    if ctx.is_json() {
        print_check_json(
            ctx,
            &graph,
            &db_root,
            &broken_file,
            &broken_attachment,
            &filetags_issues,
            &heading_backlinks,
            show_id,
        )?;
    } else {
        print_check_text(
            &graph,
            &db_root,
            &broken_file,
            &broken_attachment,
            &filetags_issues,
            &heading_backlinks,
            show_id,
            show_file,
            show_attach,
            show_filetags,
        );
    }

    if healthy {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

#[allow(clippy::too_many_arguments)]
fn print_check_json(
    ctx: &OutputContext,
    graph: &Graph,
    db_root: &Path,
    broken_file: &[BrokenFileLinkEntry],
    broken_attachment: &[BrokenAttachmentLinkEntry],
    filetags_issues: &[FiletagsIssue],
    heading_backlinks: &[HeadingBacklinkEntry],
    show_id: bool,
) -> Result<()> {
    let stats = graph.stats();
    let broken = if show_id {
        graph
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
            .collect()
    } else {
        vec![]
    };

    let failed = graph
        .parse_errors
        .iter()
        .map(|(path, err)| FailedFileEntry {
            path: path.to_string_lossy().to_string(),
            error: err.clone(),
        })
        .collect();

    let has_filetags_issues = !filetags_issues.is_empty();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && !has_filetags_issues;

    let output = CheckOutput {
        db_root: db_root.to_string_lossy().to_string(),
        stats: stats.clone(),
        duplicates: graph.duplicates.clone(),
        broken_links: broken,
        broken_file_links: broken_file.to_vec(),
        broken_attachment_links: broken_attachment.to_vec(),
        failed_files: failed,
        filetags_issues: filetags_issues.to_vec(),
        heading_backlinks: heading_backlinks.to_vec(),
        healthy,
    };
    ctx.print_json(&output)
}

#[allow(clippy::too_many_arguments)]
fn print_check_text(
    graph: &Graph,
    db_root: &Path,
    broken_file: &[BrokenFileLinkEntry],
    broken_attachment: &[BrokenAttachmentLinkEntry],
    filetags_issues: &[FiletagsIssue],
    heading_backlinks: &[HeadingBacklinkEntry],
    show_id: bool,
    show_file: bool,
    show_attach: bool,
    show_filetags: bool,
) {
    let stats = graph.stats();
    let broken_internal_count = if show_id { stats.broken_link_count } else { 0 };
    let healthy = broken_internal_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && filetags_issues.is_empty()
        && (!show_id || stats.parse_error_count == 0)
        && (!show_id || stats.duplicate_uuid_count == 0);

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

    if show_id {
        println!("  Broken links:   {}", stats.broken_link_count);
        println!("  Parse errors:   {}", stats.parse_error_count);
        println!("  Skipped files:  {}", stats.skipped_count);
        println!("  Dup UUIDs:      {}", stats.duplicate_uuid_count);
        println!("  Dup titles:     {}", stats.duplicate_title_count);
        println!("  Missing titles: {}", stats.missing_title_count);
    }

    if show_file {
        println!("  Broken files:   {}", broken_file.len());
    }
    if show_attach {
        println!("  Broken attach:  {}", broken_attachment.len());
    }
    if show_filetags {
        println!("  Filetags issues: {}", filetags_issues.len());
    }

    if show_id {
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
    }

    if show_id && !graph.broken_links.is_empty() {
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

    if !filetags_issues.is_empty() {
        println!();
        println!("Invalid filetags format:");
        for entry in filetags_issues {
            println!("  {} ({}): {}", entry.title, entry.path, entry.issue);
        }
    }

    if !heading_backlinks.is_empty() {
        println!();
        println!("Heading backlinks:");
        for entry in heading_backlinks {
            println!(
                "  heading {} (in \"{}\") <- {}",
                entry.heading_uuid, entry.primary_title, entry.source_title
            );
        }
    }

    println!();
    if healthy {
        println!("Status: healthy");
    } else {
        println!("Status: issues found");
    }
}
