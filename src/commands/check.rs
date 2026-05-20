use crate::config::ResolvedConfig;
use crate::graph::{
    DuplicateInfo, Graph, GraphStats, OverlinkEntry, SelfLinkEntry, file_link_target_exists,
};
use crate::output::OutputContext;
use crate::parser::{Link, parse_note, validate_filetags_format};
use crate::util;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_links: Option<Vec<SelfLinkEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlinks: Option<Vec<OverlinkEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_links: Option<CrossLinkResult>,
    pub healthy: bool,
}

#[derive(Clone, Serialize)]
pub struct CrossLinkResult {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
    pub target_title: String,
    pub source_to_target: usize,
    pub target_to_source: usize,
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

pub struct CheckOptions {
    pub stats: bool,
    pub file_links: bool,
    pub attachment_links: bool,
    pub id_links: bool,
    pub filetags: bool,
    pub agenda: bool,
    pub self_links: bool,
    pub overlinks: bool,
    pub cross_links: Option<Vec<String>>,
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &CheckOptions) -> Result<ExitCode> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    let cross_links_specified = opts.cross_links.is_some();
    let any_explicit = opts.stats
        || opts.id_links
        || opts.file_links
        || opts.attachment_links
        || opts.filetags
        || opts.agenda
        || opts.self_links
        || opts.overlinks
        || cross_links_specified;
    let show_stats = opts.stats || !any_explicit;
    let show_id = opts.id_links || !any_explicit;
    let show_file = opts.file_links || !any_explicit;
    let show_attach = opts.attachment_links || !any_explicit;
    let show_filetags = opts.filetags || !any_explicit;
    let show_agenda = opts.agenda || !any_explicit;
    let show_self_links = opts.self_links || !any_explicit;
    let show_overlinks = opts.overlinks || !any_explicit;

    let mut broken_file = Vec::new();
    let mut broken_attachment = Vec::new();

    if show_file {
        for node in graph.nodes.values() {
            for link in &node.outgoing {
                if let Link::File(target) = link
                    && !file_link_target_exists(target, &node.path, db_root)
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
                if let Link::Attachment(target) = link {
                    let attach_path = util::resolve_attachment_path(db_root, &node.uuid, target);
                    if !attach_path.exists() {
                        broken_attachment.push(BrokenAttachmentLinkEntry {
                            source_uuid: node.uuid.clone(),
                            source_title: node.title.clone(),
                            target_path: target.clone(),
                        });
                    }
                }
            }
        }
    }

    let mut filetags_issues = Vec::new();
    if show_filetags {
        for node in graph.nodes.values() {
            if let Some(result) = graph.results.iter().find(|r| r.path == node.path)
                && let Some(ref content) = result.raw_content
            {
                for (raw, reason) in validate_filetags_format(content) {
                    filetags_issues.push(FiletagsIssue {
                        path: node.path.display().to_string(),
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
            if let Some(result) = graph.results.iter().find(|r| r.path == node.path)
                && let Some(ref content) = result.raw_content
            {
                let parsed = parse_note(content);
                let planned_count = parsed
                    .headings
                    .iter()
                    .filter(|h| {
                        h.todo_state.is_some() && (h.scheduled.is_some() || h.deadline.is_some())
                    })
                    .count();
                if planned_count > 0 {
                    agenda_issues.push(AgendaIssue {
                        path: node.path.display().to_string(),
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

    let self_link_entries = if show_self_links {
        graph.detect_self_links(db_root)
    } else {
        vec![]
    };

    let overlink_entries = if show_overlinks {
        graph.detect_overlinks()
    } else {
        vec![]
    };

    let cross_link_result = if let Some(ref pair) = opts.cross_links {
        let node_a = graph.resolve_target(&pair[0])?;
        let node_b = graph.resolve_target(&pair[1])?;
        let a_to_b = node_a
            .outgoing
            .iter()
            .filter(|l| matches!(l, Link::Internal(u) if u == &node_b.uuid))
            .count();
        let b_to_a = node_b
            .outgoing
            .iter()
            .filter(|l| matches!(l, Link::Internal(u) if u == &node_a.uuid))
            .count();
        Some(CrossLinkResult {
            source_uuid: node_a.uuid.clone(),
            source_title: node_a.title.clone(),
            target_uuid: node_b.uuid.clone(),
            target_title: node_b.title.clone(),
            source_to_target: a_to_b,
            target_to_source: b_to_a,
        })
    } else {
        None
    };

    let healthy = graph.stats().broken_link_count == 0
        && graph.stats().parse_error_count == 0
        && graph.stats().duplicate_uuid_count == 0
        && broken_file.is_empty()
        && broken_attachment.is_empty()
        && filetags_issues.is_empty()
        && (!show_agenda || agenda_issues.is_empty())
        && self_link_entries.is_empty()
        && overlink_entries.is_empty();

    let check_data = CheckData {
        graph: &graph,
        db_root,
        broken_file: &broken_file,
        broken_attachment: &broken_attachment,
        filetags_issues: &filetags_issues,
        agenda_issues: &agenda_issues,
        self_link_entries: &self_link_entries,
        overlink_entries: &overlink_entries,
        cross_link_result: &cross_link_result,
    };
    let display_opts = CheckDisplayOptions {
        show_stats,
        show_id,
        show_file,
        show_attach,
        show_filetags,
        show_agenda,
        show_self_links,
        show_overlinks,
    };

    if ctx.is_json() {
        print_check_json(ctx, &check_data, &display_opts)?;
    } else {
        print_check_text(&check_data, &display_opts);
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
    show_self_links: bool,
    show_overlinks: bool,
}

struct CheckData<'a> {
    graph: &'a Graph,
    db_root: &'a Path,
    broken_file: &'a [BrokenFileLinkEntry],
    broken_attachment: &'a [BrokenAttachmentLinkEntry],
    filetags_issues: &'a [FiletagsIssue],
    agenda_issues: &'a [AgendaIssue],
    self_link_entries: &'a [SelfLinkEntry],
    overlink_entries: &'a [OverlinkEntry],
    cross_link_result: &'a Option<CrossLinkResult>,
}

fn print_check_json(
    ctx: &OutputContext,
    data: &CheckData,
    opts: &CheckDisplayOptions,
) -> Result<()> {
    let stats = data.graph.stats();

    let broken = if opts.show_id {
        data.graph
            .broken_links
            .iter()
            .map(|(src, tgt)| BrokenLinkEntry {
                source_uuid: src.clone(),
                source_title: data
                    .graph
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
        data.graph
            .parse_errors
            .iter()
            .map(|(path, err)| FailedFileEntry {
                path: path.display().to_string(),
                error: err.clone(),
            })
            .collect()
    } else {
        vec![]
    };

    let has_filetags_issues = !data.filetags_issues.is_empty();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && data.broken_file.is_empty()
        && data.broken_attachment.is_empty()
        && !has_filetags_issues
        && (!opts.show_agenda || data.agenda_issues.is_empty())
        && data.self_link_entries.is_empty()
        && data.overlink_entries.is_empty();

    let output = CheckOutput {
        db_root: data.db_root.display().to_string(),
        stats: if opts.show_stats {
            Some(stats.clone())
        } else {
            None
        },
        duplicates: if opts.show_id {
            Some(data.graph.duplicates.clone())
        } else {
            None
        },
        broken_links: if opts.show_id { Some(broken) } else { None },
        broken_file_links: if opts.show_file {
            Some(data.broken_file.to_vec())
        } else {
            None
        },
        broken_attachment_links: if opts.show_attach {
            Some(data.broken_attachment.to_vec())
        } else {
            None
        },
        failed_files: if opts.show_id { Some(failed) } else { None },
        filetags_issues: if opts.show_filetags {
            Some(data.filetags_issues.to_vec())
        } else {
            None
        },
        agenda_issues: if opts.show_agenda {
            Some(data.agenda_issues.to_vec())
        } else {
            None
        },
        self_links: if opts.show_self_links {
            Some(data.self_link_entries.to_vec())
        } else {
            None
        },
        overlinks: if opts.show_overlinks {
            Some(data.overlink_entries.to_vec())
        } else {
            None
        },
        cross_links: data.cross_link_result.clone(),
        healthy,
    };
    ctx.print_json(&output)
}

fn print_check_text(data: &CheckData, opts: &CheckDisplayOptions) {
    let stats = data.graph.stats();
    let healthy = stats.broken_link_count == 0
        && stats.parse_error_count == 0
        && stats.duplicate_uuid_count == 0
        && data.broken_file.is_empty()
        && data.broken_attachment.is_empty()
        && data.filetags_issues.is_empty()
        && (!opts.show_agenda || data.agenda_issues.is_empty())
        && data.self_link_entries.is_empty()
        && data.overlink_entries.is_empty();

    let has_any_output = opts.show_stats
        || opts.show_file
        || opts.show_attach
        || opts.show_filetags
        || opts.show_id
        || opts.show_agenda
        || opts.show_self_links
        || opts.show_overlinks
        || data.cross_link_result.is_some();

    if has_any_output {
        println!("Database: {}", data.db_root.display());
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
        println!("  Broken files:   {}", data.broken_file.len());
    }
    if opts.show_attach {
        println!("  Broken attach:  {}", data.broken_attachment.len());
    }
    if opts.show_filetags {
        println!("  Filetags issues: {}", data.filetags_issues.len());
    }
    if opts.show_overlinks {
        println!("  Overlinks:      {}", data.overlink_entries.len());
    }

    if opts.show_id {
        if !data.graph.duplicates.duplicate_uuids.is_empty() {
            println!();
            println!(
                "Duplicate UUIDs ({}):",
                data.graph.duplicates.duplicate_uuids.len()
            );
            for d in &data.graph.duplicates.duplicate_uuids {
                for p in &d.paths {
                    println!("  {} -> {}", d.value, p);
                }
            }
        }

        if !data.graph.duplicates.duplicate_titles.is_empty() {
            println!();
            println!(
                "Duplicate titles ({}):",
                data.graph.duplicates.duplicate_titles.len()
            );
            for d in &data.graph.duplicates.duplicate_titles {
                for p in &d.paths {
                    println!("  \"{}\" -> {}", d.value, p);
                }
            }
        }

        if !data.graph.duplicates.missing_titles.is_empty() {
            println!();
            println!(
                "Missing #+title ({}):",
                data.graph.duplicates.missing_titles.len()
            );
            for p in &data.graph.duplicates.missing_titles {
                println!("  {p}");
            }
        }
    }

    if opts.show_id && !data.graph.broken_links.is_empty() {
        println!();
        println!("Broken links ({}):", data.graph.broken_links.len());
        for (src, tgt) in &data.graph.broken_links {
            let title = data.graph.nodes.get(src).map_or("?", |n| n.title.as_str());
            println!("  {title} -> {tgt}");
        }
    }

    if !data.broken_file.is_empty() {
        println!();
        println!("Broken file links ({}):", data.broken_file.len());
        for entry in data.broken_file {
            println!("  {} -> {}", entry.source_title, entry.target_path);
        }
    }

    if !data.broken_attachment.is_empty() {
        println!();
        println!(
            "Broken attachment links ({}):",
            data.broken_attachment.len()
        );
        for entry in data.broken_attachment {
            println!("  {} -> {}", entry.source_title, entry.target_path);
        }
    }

    if !data.filetags_issues.is_empty() {
        println!();
        println!("Invalid filetags format ({}):", data.filetags_issues.len());
        for entry in data.filetags_issues {
            println!("  {} ({}): {}", entry.title, entry.path, entry.issue);
        }
    }

    if !data.agenda_issues.is_empty() {
        println!();
        println!(
            "Missing :agenda: tag (files with TODOs but no agenda tag) ({}):",
            data.agenda_issues.len()
        );
        for entry in data.agenda_issues {
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

    if !data.self_link_entries.is_empty() {
        println!();
        println!("Self-referencing links ({}):", data.self_link_entries.len());
        for entry in data.self_link_entries {
            match entry.suggestion.as_ref() {
                Some(suggestion) => println!(
                    "  {} — {} link to self: {} ({})",
                    entry.source_title, entry.link_type, entry.target, suggestion
                ),
                None => println!(
                    "  {} — {} link to self: {}",
                    entry.source_title, entry.link_type, entry.target
                ),
            }
        }
    }

    if !data.overlink_entries.is_empty() {
        println!();
        println!(
            "Overlinking (2+ links to the same note) ({}):",
            data.overlink_entries.len()
        );
        for entry in data.overlink_entries {
            println!(
                "  \"{}\" -> \"{}\" ({}x)",
                entry.source_title, entry.target_title, entry.count
            );
        }
    }

    if let Some(cr) = data.cross_link_result {
        println!();
        println!(
            "Cross-links between \"{}\" and \"{}\":",
            cr.source_title, cr.target_title
        );
        println!(
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.source_title, cr.target_title, cr.source_to_target
        );
        println!(
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.target_title, cr.source_title, cr.target_to_source
        );
    }

    if has_any_output {
        println!();
    }
    if healthy {
        println!("Status: healthy");
    } else {
        println!("Status: issues found");
    }
}
