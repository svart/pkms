use crate::config::Config;
use crate::graph::{DuplicateInfo, Graph, GraphStats};
use crate::output::OutputContext;
use crate::parser::{Link, parse_note, validate_filetags_format};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::process::ExitCode;

#[derive(Serialize)]
pub struct CheckOutput {
    pub db_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<GraphStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicates: Option<DuplicateInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_links: Option<Vec<BrokenLinkEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_file_links: Option<Vec<BrokenFileLinkEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_attachment_links: Option<Vec<BrokenAttachmentLinkEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_files: Option<Vec<FailedFileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filetags_issues: Option<Vec<FiletagsIssue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agenda_issues: Option<Vec<AgendaIssue>>,
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

#[derive(Clone, Serialize)]
pub struct AgendaIssue {
    pub path: String,
    pub title: String,
    pub uuid: String,
    pub todo_count: usize,
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

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: &Config,
    ctx: &OutputContext,
    db_cli: Option<&std::path::Path>,
    stats: bool,
    file_links: bool,
    attachment_links: bool,
    id_links: bool,
    filetags: bool,
    agenda: bool,
) -> Result<ExitCode> {
    let graph = Graph::load(config, db_cli)?;
    let db_root = config.resolve_db_root(db_cli)?;

    let any_explicit = stats || id_links || file_links || attachment_links || filetags || agenda;
    let show_stats = stats || !any_explicit;
    let show_id = id_links || !any_explicit;
    let show_file = file_links || !any_explicit;
    let show_attach = attachment_links || !any_explicit;
    let show_filetags = filetags || !any_explicit;
    let show_agenda = agenda;

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

    let mut agenda_issues = Vec::new();
    if show_agenda {
        for node in graph.nodes.values() {
            if node.filetags.iter().any(|t| t == "agenda") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&node.path) {
                let parsed = parse_note(&content);
                let planned_count = parsed
                    .headings
                    .iter()
                    .filter(|h| {
                        h.todo_state.is_some() && (h.scheduled.is_some() || h.deadline.is_some())
                    })
                    .count();
                if planned_count > 0 {
                    agenda_issues.push(AgendaIssue {
                        path: node.path.to_string_lossy().to_string(),
                        title: node.title.clone(),
                        uuid: node.uuid.clone(),
                        todo_count: planned_count,
                        issue: format!(
                            "{planned_count} planned TODO heading(s) found but :agenda: tag missing"
                        ),
                    });
                }
            }
        }
    }

    let healthy = graph.stats().broken_link_count == 0
        && graph.stats().parse_error_count == 0
        && graph.stats().duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && filetags_issues.is_empty()
        && (!show_agenda || agenda_issues.is_empty());

    if ctx.is_json() {
        print_check_json(
            ctx,
            &graph,
            &db_root,
            &broken_file,
            &broken_attachment,
            &filetags_issues,
            &agenda_issues,
            &CheckDisplayOptions {
                show_stats,
                show_id,
                show_file,
                show_attach,
                show_filetags,
                show_agenda,
            },
        )?;
    } else {
        print_check_text(
            &graph,
            &db_root,
            &broken_file,
            &broken_attachment,
            &filetags_issues,
            &agenda_issues,
            &CheckDisplayOptions {
                show_stats,
                show_id,
                show_file,
                show_attach,
                show_filetags,
                show_agenda,
            },
        );
    }

    if healthy {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

struct CheckDisplayOptions {
    show_stats: bool,
    show_id: bool,
    show_file: bool,
    show_attach: bool,
    show_filetags: bool,
    show_agenda: bool,
}

#[allow(clippy::too_many_arguments)]
fn print_check_json(
    ctx: &OutputContext,
    graph: &Graph,
    db_root: &Path,
    broken_file: &[BrokenFileLinkEntry],
    broken_attachment: &[BrokenAttachmentLinkEntry],
    filetags_issues: &[FiletagsIssue],
    agenda_issues: &[AgendaIssue],
    opts: &CheckDisplayOptions,
) -> Result<()> {
    let stats = graph.stats();

    let broken = if opts.show_id {
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

    let failed = if opts.show_id {
        graph
            .parse_errors
            .iter()
            .map(|(path, err)| FailedFileEntry {
                path: path.to_string_lossy().to_string(),
                error: err.clone(),
            })
            .collect()
    } else {
        vec![]
    };

    let has_filetags_issues = !filetags_issues.is_empty();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && !has_filetags_issues
        && (!opts.show_agenda || agenda_issues.is_empty());

    let output = CheckOutput {
        db_root: db_root.to_string_lossy().to_string(),
        stats: if opts.show_stats {
            Some(stats.clone())
        } else {
            None
        },
        duplicates: if opts.show_id {
            Some(graph.duplicates.clone())
        } else {
            None
        },
        broken_links: if opts.show_id { Some(broken) } else { None },
        broken_file_links: if opts.show_file {
            Some(broken_file.to_vec())
        } else {
            None
        },
        broken_attachment_links: if opts.show_attach {
            Some(broken_attachment.to_vec())
        } else {
            None
        },
        failed_files: if opts.show_id { Some(failed) } else { None },
        filetags_issues: if opts.show_filetags {
            Some(filetags_issues.to_vec())
        } else {
            None
        },
        agenda_issues: if opts.show_agenda {
            Some(agenda_issues.to_vec())
        } else {
            None
        },
        healthy,
    };
    ctx.print_json(&output)
}

fn print_check_text(
    graph: &Graph,
    db_root: &Path,
    broken_file: &[BrokenFileLinkEntry],
    broken_attachment: &[BrokenAttachmentLinkEntry],
    filetags_issues: &[FiletagsIssue],
    agenda_issues: &[AgendaIssue],
    opts: &CheckDisplayOptions,
) {
    let stats = graph.stats();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && filetags_issues.is_empty()
        && (!opts.show_agenda || agenda_issues.is_empty());

    if opts.show_stats
        || opts.show_file
        || opts.show_attach
        || opts.show_filetags
        || opts.show_id
        || opts.show_agenda
    {
        println!("Database: {}", db_root.display());
    }

    if opts.show_stats {
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
        println!("  Parse errors:   {}", stats.parse_error_count);
        println!("  Skipped files:  {}", stats.skipped_count);
        println!("  Dup UUIDs:      {}", stats.duplicate_uuid_count);
        println!("  Dup titles:     {}", stats.duplicate_title_count);
        println!("  Missing titles: {}", stats.missing_title_count);
    }

    if opts.show_file {
        println!("  Broken files:   {}", broken_file.len());
    }
    if opts.show_attach {
        println!("  Broken attach:  {}", broken_attachment.len());
    }
    if opts.show_filetags {
        println!("  Filetags issues: {}", filetags_issues.len());
    }

    if opts.show_id {
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

    if opts.show_id && !graph.broken_links.is_empty() {
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

    if !agenda_issues.is_empty() {
        println!();
        println!("Missing :agenda: tag (files with TODOs but no agenda tag):");
        for entry in agenda_issues {
            let short_uuid = if entry.uuid.len() >= 8 {
                &entry.uuid[..8]
            } else {
                &entry.uuid
            };
            println!(
                "  {} ({})  \u{2014} {} TODOs",
                entry.title, short_uuid, entry.todo_count
            );
        }
    }

    if opts.show_stats
        || opts.show_file
        || opts.show_attach
        || opts.show_filetags
        || opts.show_id
        || opts.show_agenda
    {
        println!();
    }
    if healthy {
        println!("Status: healthy");
    } else {
        println!("Status: issues found");
    }
}
