use crate::cli::CheckArgs;
use crate::config::ResolvedConfig;
use crate::graph::{DuplicateInfo, Graph, GraphStats, OverlinkEntry, SelfLinkEntry};
use crate::link_check::{
    LinkCheckBrokenTarget, LinkCheckJob, LinkCheckKind, run_local_link_checks_sequential,
    sort_link_check_jobs,
};
use crate::output::OutputContext;
use crate::parser::{Link, validate_filetags_format};
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;
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

pub struct CheckOptions {
    pub stats: bool,
    pub file_links: bool,
    pub attachment_links: bool,
    pub id_links: bool,
    pub filetags: bool,
    pub self_links: bool,
    pub overlinks: bool,
    pub cross_links: Option<Vec<String>>,
}

pub struct CheckCommandOutput {
    pub output: CheckOutput,
    pub exit_code: ExitCode,
}

impl From<&CheckArgs> for CheckOptions {
    fn from(args: &CheckArgs) -> Self {
        CheckOptions {
            stats: args.stats,
            file_links: args.file_links,
            attachment_links: args.attachment_links,
            id_links: args.id_links,
            filetags: args.filetags,
            self_links: args.self_links,
            overlinks: args.overlinks,
            cross_links: args.cross_links.clone(),
        }
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &CheckOptions) -> Result<ExitCode> {
    let output = execute(config, opts)?;
    render(ctx, &output)
}

pub fn execute(config: &ResolvedConfig, opts: &CheckOptions) -> Result<CheckCommandOutput> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    let display_opts = CheckDisplayOptions::from_options(opts);
    let issue_data = collect_check_data(&graph, db_root, opts, &display_opts)?;
    let output = build_check_output(&issue_data, &display_opts);
    let exit_code = if output.healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };

    Ok(CheckCommandOutput { output, exit_code })
}

pub fn render(ctx: &OutputContext, output: &CheckCommandOutput) -> Result<ExitCode> {
    if ctx.is_json() {
        ctx.print_json(&output.output)?;
    } else {
        print!("{}", render_text(&output.output));
    }

    Ok(output.exit_code)
}

impl CheckDisplayOptions {
    fn from_options(opts: &CheckOptions) -> Self {
        let cross_links_specified = opts.cross_links.is_some();
        let any_explicit = opts.stats
            || opts.id_links
            || opts.file_links
            || opts.attachment_links
            || opts.filetags
            || opts.self_links
            || opts.overlinks
            || cross_links_specified;

        CheckDisplayOptions {
            show_stats: opts.stats || !any_explicit,
            show_id: opts.id_links || !any_explicit,
            show_file: opts.file_links || !any_explicit,
            show_attach: opts.attachment_links || !any_explicit,
            show_filetags: opts.filetags || !any_explicit,
            show_self_links: opts.self_links || !any_explicit,
            show_overlinks: opts.overlinks || !any_explicit,
        }
    }
}

fn collect_check_data<'a>(
    graph: &'a Graph,
    db_root: &'a Path,
    opts: &CheckOptions,
    display_opts: &CheckDisplayOptions,
) -> Result<CheckData<'a>> {
    let cross_links_specified = opts.cross_links.is_some();

    let link_jobs =
        collect_local_link_check_jobs(graph, display_opts.show_file, display_opts.show_attach);
    let (broken_file, broken_attachment) =
        split_broken_link_targets(run_local_link_checks_sequential(link_jobs, db_root));

    let mut filetags_issues = Vec::new();
    if display_opts.show_filetags {
        for result in &graph.results {
            if let Some(ref content) = result.raw_content {
                let node = graph
                    .path_to_uuid
                    .get(&result.path)
                    .and_then(|uuid| graph.nodes.get(uuid));
                for (raw, reason) in validate_filetags_format(content) {
                    filetags_issues.push(FiletagsIssue {
                        path: result.path.display().to_string(),
                        title: node
                            .map(|node| node.title.clone())
                            .or_else(|| result.parsed.title.clone())
                            .unwrap_or_default(),
                        issue: format!("tag '{raw}' — {reason}"),
                    });
                }
            }
        }
    }

    let self_link_entries = if display_opts.show_self_links {
        graph.detect_self_links(db_root)
    } else {
        vec![]
    };

    let overlink_entries = if display_opts.show_overlinks {
        graph.detect_overlinks()
    } else {
        vec![]
    };

    let cross_link_result = if cross_links_specified && let Some(ref pair) = opts.cross_links {
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

    Ok(CheckData {
        graph,
        db_root,
        broken_file,
        broken_attachment,
        filetags_issues,
        self_link_entries,
        overlink_entries,
        cross_link_result,
    })
}

fn collect_local_link_check_jobs(
    graph: &Graph,
    include_files: bool,
    include_attachments: bool,
) -> Vec<LinkCheckJob> {
    let mut jobs = Vec::new();
    for node in graph.nodes.values() {
        for link in &node.outgoing {
            match link {
                Link::File(target) if include_files => jobs.push(LinkCheckJob::file(
                    node.uuid.clone(),
                    node.title.clone(),
                    node.path.clone(),
                    target.clone(),
                )),
                Link::Attachment(target) if include_attachments => {
                    jobs.push(LinkCheckJob::attachment(
                        node.uuid.clone(),
                        node.title.clone(),
                        node.path.clone(),
                        target.clone(),
                    ));
                }
                _ => {}
            }
        }
    }
    sort_link_check_jobs(&mut jobs);
    jobs
}

fn split_broken_link_targets(
    broken_targets: Vec<LinkCheckBrokenTarget>,
) -> (Vec<BrokenFileLinkEntry>, Vec<BrokenAttachmentLinkEntry>) {
    let mut broken_file = Vec::new();
    let mut broken_attachment = Vec::new();

    for target in broken_targets {
        match target.kind {
            LinkCheckKind::File => broken_file.push(BrokenFileLinkEntry {
                source_uuid: target.source_uuid,
                source_title: target.source_title,
                target_path: target.target,
            }),
            LinkCheckKind::Attachment => broken_attachment.push(BrokenAttachmentLinkEntry {
                source_uuid: target.source_uuid,
                source_title: target.source_title,
                target_path: target.target,
            }),
        }
    }

    (broken_file, broken_attachment)
}

struct CheckDisplayOptions {
    show_stats: bool,
    show_id: bool,
    show_file: bool,
    show_attach: bool,
    show_filetags: bool,
    show_self_links: bool,
    show_overlinks: bool,
}

struct CheckData<'a> {
    graph: &'a Graph,
    db_root: &'a Path,
    broken_file: Vec<BrokenFileLinkEntry>,
    broken_attachment: Vec<BrokenAttachmentLinkEntry>,
    filetags_issues: Vec<FiletagsIssue>,
    self_link_entries: Vec<SelfLinkEntry>,
    overlink_entries: Vec<OverlinkEntry>,
    cross_link_result: Option<CrossLinkResult>,
}

impl CheckData<'_> {
    fn is_healthy(&self) -> bool {
        let stats = self.graph.stats();
        stats.broken_link_count == 0
            && stats.parse_error_count == 0
            && stats.duplicate_uuid_count == 0
            && self.broken_file.is_empty()
            && self.broken_attachment.is_empty()
            && self.filetags_issues.is_empty()
            && self.self_link_entries.is_empty()
            && self.overlink_entries.is_empty()
    }
}

fn build_check_output(data: &CheckData, opts: &CheckDisplayOptions) -> CheckOutput {
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

    let healthy = data.is_healthy();

    CheckOutput {
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
    }
}

pub fn render_text(output: &CheckOutput) -> String {
    let mut text = String::new();

    let has_any_output = output.stats.is_some()
        || output.broken_file_links.is_some()
        || output.broken_attachment_links.is_some()
        || output.filetags_issues.is_some()
        || output.duplicates.is_some()
        || output.broken_links.is_some()
        || output.failed_files.is_some()
        || output.self_links.is_some()
        || output.overlinks.is_some()
        || output.cross_links.is_some();

    if has_any_output {
        let _ = writeln!(text, "Database: {}", output.db_root);
    }

    if let Some(stats) = &output.stats {
        let _ = writeln!(text, "  Notes:          {}", stats.total_notes);
        let _ = writeln!(
            text,
            "  Links:          {} (internal: {}, file: {}, url: {})",
            stats.total_links,
            stats.total_internal_links,
            stats.total_file_links,
            stats.total_url_links,
        );
        let _ = writeln!(text, "  Orphans:        {}", stats.orphan_notes);
        let _ = writeln!(text, "  Broken links:   {}", stats.broken_link_count);
        let _ = writeln!(text, "  Parse errors:   {}", stats.parse_error_count);
        let _ = writeln!(text, "  Skipped files:  {}", stats.skipped_count);
        let _ = writeln!(text, "  Dup UUIDs:      {}", stats.duplicate_uuid_count);
        let _ = writeln!(text, "  Dup titles:     {}", stats.duplicate_title_count);
        let _ = writeln!(text, "  Missing titles: {}", stats.missing_title_count);
    }

    if let Some(broken_file) = &output.broken_file_links {
        let _ = writeln!(text, "  Broken files:   {}", broken_file.len());
    }
    if let Some(broken_attachment) = &output.broken_attachment_links {
        let _ = writeln!(text, "  Broken attach:  {}", broken_attachment.len());
    }
    if let Some(filetags_issues) = &output.filetags_issues {
        let _ = writeln!(text, "  Filetags issues: {}", filetags_issues.len());
    }
    if let Some(overlinks) = &output.overlinks {
        let _ = writeln!(text, "  Overlinks:      {}", overlinks.len());
    }

    if let Some(duplicates) = &output.duplicates {
        if !duplicates.duplicate_uuids.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Duplicate UUIDs ({}):",
                duplicates.duplicate_uuids.len()
            );
            for d in &duplicates.duplicate_uuids {
                for p in &d.paths {
                    let _ = writeln!(text, "  {} -> {}", d.value, p);
                }
            }
        }

        if !duplicates.duplicate_titles.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Duplicate titles ({}):",
                duplicates.duplicate_titles.len()
            );
            for d in &duplicates.duplicate_titles {
                for p in &d.paths {
                    let _ = writeln!(text, "  \"{}\" -> {}", d.value, p);
                }
            }
        }

        if !duplicates.missing_titles.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Missing #+title ({}):",
                duplicates.missing_titles.len()
            );
            for p in &duplicates.missing_titles {
                let _ = writeln!(text, "  {p}");
            }
        }
    }

    if let Some(broken_links) = &output.broken_links
        && !broken_links.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Broken links ({}):", broken_links.len());
        for entry in broken_links {
            let title = if entry.source_title.is_empty() {
                "?"
            } else {
                entry.source_title.as_str()
            };
            let _ = writeln!(text, "  {title} -> {}", entry.target_uuid);
        }
    }

    if let Some(broken_file) = &output.broken_file_links
        && !broken_file.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Broken file links ({}):", broken_file.len());
        for entry in broken_file {
            let _ = writeln!(text, "  {} -> {}", entry.source_title, entry.target_path);
        }
    }

    if let Some(broken_attachment) = &output.broken_attachment_links
        && !broken_attachment.is_empty()
    {
        text.push('\n');
        let _ = writeln!(
            text,
            "Broken attachment links ({}):",
            broken_attachment.len()
        );
        for entry in broken_attachment {
            let _ = writeln!(text, "  {} -> {}", entry.source_title, entry.target_path);
        }
    }

    if let Some(filetags_issues) = &output.filetags_issues
        && !filetags_issues.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Invalid filetags format ({}):", filetags_issues.len());
        for entry in filetags_issues {
            let _ = writeln!(text, "  {} ({}): {}", entry.title, entry.path, entry.issue);
        }
    }

    if let Some(self_links) = &output.self_links
        && !self_links.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Self-referencing links ({}):", self_links.len());
        for entry in self_links {
            match entry.suggestion.as_ref() {
                Some(suggestion) => {
                    let _ = writeln!(
                        text,
                        "  {} — {} link to self: {} ({})",
                        entry.source_title, entry.link_type, entry.target, suggestion
                    );
                }
                None => {
                    let _ = writeln!(
                        text,
                        "  {} — {} link to self: {}",
                        entry.source_title, entry.link_type, entry.target
                    );
                }
            }
        }
    }

    if let Some(overlinks) = &output.overlinks
        && !overlinks.is_empty()
    {
        text.push('\n');
        let _ = writeln!(
            text,
            "Overlinking (2+ links to the same note) ({}):",
            overlinks.len()
        );
        for entry in overlinks {
            let _ = writeln!(
                text,
                "  \"{}\" -> \"{}\" ({}x)",
                entry.source_title, entry.target_title, entry.count
            );
        }
    }

    if let Some(cr) = &output.cross_links {
        text.push('\n');
        let _ = writeln!(
            text,
            "Cross-links between \"{}\" and \"{}\":",
            cr.source_title, cr.target_title
        );
        let _ = writeln!(
            text,
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.source_title, cr.target_title, cr.source_to_target
        );
        let _ = writeln!(
            text,
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.target_title, cr.source_title, cr.target_to_source
        );
    }

    if has_any_output {
        text.push('\n');
    }
    if output.healthy {
        text.push_str("Status: healthy\n");
    } else {
        text.push_str("Status: issues found\n");
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphStats;

    fn healthy_output() -> CheckOutput {
        CheckOutput {
            db_root: "/notes".to_string(),
            stats: Some(GraphStats {
                total_notes: 2,
                total_links: 3,
                total_internal_links: 1,
                total_file_links: 1,
                total_url_links: 1,
                orphan_notes: 0,
                broken_link_count: 0,
                skipped_count: 0,
                parse_error_count: 0,
                duplicate_uuid_count: 0,
                duplicate_title_count: 0,
                missing_title_count: 0,
            }),
            duplicates: None,
            broken_links: None,
            broken_file_links: None,
            broken_attachment_links: None,
            failed_files: None,
            filetags_issues: None,
            self_links: None,
            overlinks: None,
            cross_links: None,
            healthy: true,
        }
    }

    #[test]
    fn renders_healthy_text_from_typed_output() {
        let text = render_text(&healthy_output());

        assert!(text.contains("Database: /notes"));
        assert!(text.contains("  Notes:          2"));
        assert!(text.contains("  Links:          3 (internal: 1, file: 1, url: 1)"));
        assert!(text.ends_with("Status: healthy\n"));
    }

    #[test]
    fn renders_issue_text_from_typed_output() {
        let mut output = healthy_output();
        output.stats = None;
        output.broken_file_links = Some(vec![BrokenFileLinkEntry {
            source_uuid: "source".to_string(),
            source_title: "Source Note".to_string(),
            target_path: "missing.org".to_string(),
        }]);
        output.healthy = false;

        let text = render_text(&output);

        assert!(text.contains("  Broken files:   1"));
        assert!(text.contains("Broken file links (1):"));
        assert!(text.contains("  Source Note -> missing.org"));
        assert!(text.ends_with("Status: issues found\n"));
    }

    #[test]
    fn collects_only_requested_local_link_check_jobs_in_stable_order() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        std::fs::write(
            db_root.join("beta.org"),
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta

[[file:beta-missing.org]]
[[attachment:beta.png]]
[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc]]
[[https://example.com]]
"#,
        )
        .unwrap();
        std::fs::write(
            db_root.join("alpha.org"),
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[attachment:alpha.png]]
[[file:alpha-missing.org]]
"#,
        )
        .unwrap();
        let config = ResolvedConfig::for_test_db(db_root);
        let graph = Graph::load(&config).unwrap();

        let all_jobs = collect_local_link_check_jobs(&graph, true, true);
        let all_observed: Vec<_> = all_jobs
            .iter()
            .map(|job| {
                (
                    job.kind,
                    job.source_uuid.as_str(),
                    job.source_title.as_str(),
                    job.target.as_str(),
                )
            })
            .collect();
        assert_eq!(
            all_observed,
            vec![
                (
                    LinkCheckKind::File,
                    "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                    "Alpha",
                    "alpha-missing.org",
                ),
                (
                    LinkCheckKind::File,
                    "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                    "Beta",
                    "beta-missing.org",
                ),
                (
                    LinkCheckKind::Attachment,
                    "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                    "Alpha",
                    "alpha.png",
                ),
                (
                    LinkCheckKind::Attachment,
                    "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                    "Beta",
                    "beta.png",
                ),
            ]
        );

        let file_jobs = collect_local_link_check_jobs(&graph, true, false);
        assert_eq!(file_jobs.len(), 2);
        assert!(file_jobs.iter().all(|job| job.kind == LinkCheckKind::File));

        let attachment_jobs = collect_local_link_check_jobs(&graph, false, true);
        assert_eq!(attachment_jobs.len(), 2);
        assert!(
            attachment_jobs
                .iter()
                .all(|job| job.kind == LinkCheckKind::Attachment)
        );
    }
}
