use crate::cli::CheckArgs;
use crate::config::ResolvedConfig;
use crate::graph::validation::{GraphValidationIssues, GraphValidationOptions};
use crate::graph::{DuplicateInfo, Graph, GraphStats, OverlinkEntry, SelfLinkEntry};
use crate::link_check::{
    LinkCheckErrorTarget, LinkCheckKind, LinkCheckResults, SshFileCheckOptions,
    run_local_link_checks, run_ssh_link_checks,
};
use crate::output::OutputContext;
use crate::parser::Link;
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
    pub file_link_errors: Option<Vec<FileLinkErrorEntry>>,
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
pub struct FileLinkErrorEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_path: String,
    pub backend: String,
    pub error_kind: String,
    pub message: String,
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
    pub remote_file_links: bool,
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
            remote_file_links: args.remote_file_links,
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
    ensure_remote_file_links_available(opts.remote_file_links)?;

    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    let display_opts = CheckDisplayOptions::from_options(opts);
    let issue_data = collect_check_data(config, &graph, db_root, opts, &display_opts)?;
    let output = build_check_output(&issue_data, &display_opts);
    let exit_code = if output.healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };

    Ok(CheckCommandOutput { output, exit_code })
}

#[cfg(feature = "ssh")]
fn ensure_remote_file_links_available(_requested: bool) -> Result<()> {
    Ok(())
}

#[cfg(not(feature = "ssh"))]
fn ensure_remote_file_links_available(requested: bool) -> Result<()> {
    if requested {
        anyhow::bail!(
            "SSH file-link checks are not available in this build. Rebuild with --features ssh."
        )
    }
    Ok(())
}

pub fn render(ctx: &OutputContext, output: &CheckCommandOutput) -> Result<ExitCode> {
    if ctx.is_structured() {
        ctx.print_structured(&output.output)?;
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
            || opts.remote_file_links
            || opts.attachment_links
            || opts.filetags
            || opts.self_links
            || opts.overlinks
            || cross_links_specified;

        CheckDisplayOptions {
            show_stats: opts.stats || !any_explicit,
            show_id: opts.id_links || !any_explicit,
            show_file: opts.file_links || opts.remote_file_links || !any_explicit,
            show_attach: opts.attachment_links || !any_explicit,
            show_filetags: opts.filetags || !any_explicit,
            show_self_links: opts.self_links || !any_explicit,
            show_overlinks: opts.overlinks || !any_explicit,
        }
    }
}

fn collect_check_data<'a>(
    config: &'a ResolvedConfig,
    graph: &'a Graph,
    db_root: &'a Path,
    opts: &CheckOptions,
    display_opts: &CheckDisplayOptions,
) -> Result<CheckData<'a>> {
    let cross_links_specified = opts.cross_links.is_some();

    let link_jobs =
        graph.collect_local_link_check_jobs(display_opts.show_file, display_opts.show_attach);
    let mut link_results = run_local_link_checks(link_jobs, db_root);
    if opts.remote_file_links {
        let ssh_jobs = graph.collect_ssh_file_link_check_jobs();
        let ssh_options = SshFileCheckOptions::from_config(config.ssh.as_ref());
        link_results.extend(run_ssh_link_checks(ssh_jobs, &ssh_options));
    }
    let (broken_file, file_link_errors, broken_attachment) = split_link_check_results(link_results);

    let validation_issues = graph.collect_validation_issues(
        db_root,
        &GraphValidationOptions {
            internal_links: display_opts.show_id,
            filetags: display_opts.show_filetags,
            duplicates: display_opts.show_id,
            self_links: display_opts.show_self_links,
            overlinks: display_opts.show_overlinks,
        },
    );

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
        file_link_errors,
        broken_attachment,
        validation_issues,
        cross_link_result,
    })
}

fn split_link_check_results(
    results: LinkCheckResults,
) -> (
    Vec<BrokenFileLinkEntry>,
    Vec<FileLinkErrorEntry>,
    Vec<BrokenAttachmentLinkEntry>,
) {
    let mut broken_file = Vec::new();
    let mut file_link_errors = Vec::new();
    let mut broken_attachment = Vec::new();

    for target in results.broken {
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

    for error in results.errors {
        if error.kind == LinkCheckKind::File {
            file_link_errors.push(file_link_error_entry(error));
        }
    }

    (broken_file, file_link_errors, broken_attachment)
}

fn file_link_error_entry(error: LinkCheckErrorTarget) -> FileLinkErrorEntry {
    FileLinkErrorEntry {
        source_uuid: error.source_uuid,
        source_title: error.source_title,
        target_path: error.target,
        backend: error.backend.as_str().to_string(),
        error_kind: error.error_kind,
        message: error.message,
    }
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
    file_link_errors: Vec<FileLinkErrorEntry>,
    broken_attachment: Vec<BrokenAttachmentLinkEntry>,
    validation_issues: GraphValidationIssues,
    cross_link_result: Option<CrossLinkResult>,
}

impl CheckData<'_> {
    fn is_healthy(&self, opts: &CheckDisplayOptions) -> bool {
        let stats = self.graph.stats();
        let id_healthy = !opts.show_id
            || (stats.broken_link_count == 0
                && stats.parse_error_count == 0
                && stats.duplicate_uuid_count == 0);
        let stats_healthy = !opts.show_stats
            || (stats.broken_link_count == 0
                && stats.parse_error_count == 0
                && stats.duplicate_uuid_count == 0);
        id_healthy
            && stats_healthy
            && (!opts.show_file
                || (self.broken_file.is_empty() && self.file_link_errors.is_empty()))
            && (!opts.show_attach || self.broken_attachment.is_empty())
            && (!opts.show_filetags || self.validation_issues.filetags.is_empty())
            && (!opts.show_self_links || self.validation_issues.self_links.is_empty())
            && (!opts.show_overlinks || self.validation_issues.overlinks.is_empty())
    }
}

fn build_check_output(data: &CheckData, opts: &CheckDisplayOptions) -> CheckOutput {
    let stats = data.graph.stats();

    let broken = if opts.show_id {
        data.validation_issues
            .broken_internal_links
            .iter()
            .map(|issue| BrokenLinkEntry {
                source_uuid: issue.source_uuid.clone(),
                source_title: issue.source_title.clone(),
                target_uuid: issue.target_uuid.clone(),
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

    let healthy = data.is_healthy(opts);

    CheckOutput {
        db_root: data.db_root.display().to_string(),
        stats: if opts.show_stats {
            Some(stats.clone())
        } else {
            None
        },
        duplicates: if opts.show_id {
            data.validation_issues.duplicates.clone()
        } else {
            None
        },
        broken_links: if opts.show_id { Some(broken) } else { None },
        broken_file_links: if opts.show_file {
            Some(data.broken_file.to_vec())
        } else {
            None
        },
        file_link_errors: if opts.show_file {
            Some(data.file_link_errors.to_vec())
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
            Some(
                data.validation_issues
                    .filetags
                    .iter()
                    .map(|issue| FiletagsIssue {
                        path: issue.path.display().to_string(),
                        title: issue.title.clone(),
                        issue: format!("tag '{}' — {}", issue.raw, issue.reason),
                    })
                    .collect(),
            )
        } else {
            None
        },
        self_links: if opts.show_self_links {
            Some(data.validation_issues.self_links.to_vec())
        } else {
            None
        },
        overlinks: if opts.show_overlinks {
            Some(data.validation_issues.overlinks.to_vec())
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
        || output.file_link_errors.is_some()
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
    if let Some(file_link_errors) = &output.file_link_errors {
        let _ = writeln!(text, "  File errors:    {}", file_link_errors.len());
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

    if let Some(file_link_errors) = &output.file_link_errors
        && !file_link_errors.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "File link errors ({}):", file_link_errors.len());
        for entry in file_link_errors {
            let _ = writeln!(
                text,
                "  {} -> {} [{}:{}] {}",
                entry.source_title,
                entry.target_path,
                entry.backend,
                entry.error_kind,
                entry.message
            );
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
            file_link_errors: None,
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
    fn renders_file_link_errors_from_typed_output() {
        let mut output = healthy_output();
        output.stats = None;
        output.file_link_errors = Some(vec![FileLinkErrorEntry {
            source_uuid: "source".to_string(),
            source_title: "Source Note".to_string(),
            target_path: "/ssh:example.org:/tmp/file.txt".to_string(),
            backend: "ssh".to_string(),
            error_kind: "host_key".to_string(),
            message: "known host mismatch".to_string(),
        }]);
        output.healthy = false;

        let text = render_text(&output);

        assert!(text.contains("  File errors:    1"));
        assert!(text.contains("File link errors (1):"));
        assert!(text.contains(
            "  Source Note -> /ssh:example.org:/tmp/file.txt [ssh:host_key] known host mismatch"
        ));
        assert!(text.ends_with("Status: issues found\n"));
    }

    #[test]
    fn targeted_file_link_check_health_ignores_hidden_id_link_issues() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("alpha.org"),
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[id:bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb]]
"#,
        )
        .unwrap();
        let config = ResolvedConfig::for_test_db(dir.path());
        let output = execute(
            &config,
            &CheckOptions {
                stats: false,
                file_links: true,
                remote_file_links: false,
                attachment_links: false,
                id_links: false,
                filetags: false,
                self_links: false,
                overlinks: false,
                cross_links: None,
            },
        )
        .unwrap();

        assert!(output.output.healthy);
        assert!(output.output.broken_links.is_none());
        assert_eq!(output.output.broken_file_links.unwrap().len(), 0);
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

        let all_jobs = graph.collect_local_link_check_jobs(true, true);
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

        let file_jobs = graph.collect_local_link_check_jobs(true, false);
        assert_eq!(file_jobs.len(), 2);
        assert!(file_jobs.iter().all(|job| job.kind == LinkCheckKind::File));

        let attachment_jobs = graph.collect_local_link_check_jobs(false, true);
        assert_eq!(attachment_jobs.len(), 2);
        assert!(
            attachment_jobs
                .iter()
                .all(|job| job.kind == LinkCheckKind::Attachment)
        );
    }
}
