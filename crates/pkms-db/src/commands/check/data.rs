use super::CheckConfig;
use super::model::{
    BrokenAttachmentLinkEntry, BrokenFileLinkEntry, BrokenLinkEntry, CheckItem, CheckOptions,
    CheckOutput, CheckSelection, CrossLinkResult, FailedFileEntry, FileLinkErrorEntry,
    FiletagsIssue,
};
use crate::link_check::{
    LinkCheckErrorTarget, LinkCheckKind, LinkCheckResults, SshFileCheckOptions,
    run_local_link_checks, run_ssh_link_checks,
};
use anyhow::Result;
use pkms_org::Graph;
use pkms_org::graph::validation::{
    GraphValidationCheck, GraphValidationIssues, GraphValidationOptions,
};
use pkms_org::parser::Link;
use std::path::Path;

pub(super) struct CheckDisplayOptions {
    sections: Vec<CheckDisplaySection>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheckDisplaySection {
    Stats,
    IdLinks,
    FileLinks,
    AttachmentLinks,
    Filetags,
    SelfLinks,
    Overlinks,
}

impl CheckDisplaySection {
    const ALL: [CheckDisplaySection; 7] = [
        CheckDisplaySection::Stats,
        CheckDisplaySection::IdLinks,
        CheckDisplaySection::FileLinks,
        CheckDisplaySection::AttachmentLinks,
        CheckDisplaySection::Filetags,
        CheckDisplaySection::SelfLinks,
        CheckDisplaySection::Overlinks,
    ];
}

impl CheckDisplayOptions {
    pub(super) fn from_options(opts: &CheckOptions) -> Self {
        CheckDisplayOptions {
            sections: CheckDisplaySection::ALL
                .iter()
                .copied()
                .filter(|section| selection_shows(&opts.checks, *section))
                .collect(),
        }
    }

    fn shows(&self, section: CheckDisplaySection) -> bool {
        self.sections.contains(&section)
    }

    fn local_link_kinds(&self) -> Vec<LinkCheckKind> {
        let mut kinds = Vec::new();
        if self.shows(CheckDisplaySection::FileLinks) {
            kinds.push(LinkCheckKind::File);
        }
        if self.shows(CheckDisplaySection::AttachmentLinks) {
            kinds.push(LinkCheckKind::Attachment);
        }
        kinds
    }

    fn validation_options(&self) -> GraphValidationOptions {
        let mut checks = Vec::new();
        if self.shows(CheckDisplaySection::IdLinks) {
            checks.push(GraphValidationCheck::InternalLinks);
            checks.push(GraphValidationCheck::Duplicates);
        }
        if self.shows(CheckDisplaySection::Filetags) {
            checks.push(GraphValidationCheck::Filetags);
        }
        if self.shows(CheckDisplaySection::SelfLinks) {
            checks.push(GraphValidationCheck::SelfLinks);
        }
        if self.shows(CheckDisplaySection::Overlinks) {
            checks.push(GraphValidationCheck::Overlinks);
        }
        GraphValidationOptions::new(checks)
    }
}

fn selection_shows(selection: &CheckSelection, section: CheckDisplaySection) -> bool {
    match selection {
        CheckSelection::Default => true,
        CheckSelection::Explicit(checks) => match section {
            CheckDisplaySection::Stats => checks.contains(&CheckItem::Stats),
            CheckDisplaySection::IdLinks => checks.contains(&CheckItem::IdLinks),
            CheckDisplaySection::FileLinks => {
                checks.contains(&CheckItem::FileLinks)
                    || checks.contains(&CheckItem::RemoteFileLinks)
            }
            CheckDisplaySection::AttachmentLinks => checks.contains(&CheckItem::AttachmentLinks),
            CheckDisplaySection::Filetags => checks.contains(&CheckItem::Filetags),
            CheckDisplaySection::SelfLinks => checks.contains(&CheckItem::SelfLinks),
            CheckDisplaySection::Overlinks => checks.contains(&CheckItem::Overlinks),
        },
    }
}

pub(super) fn collect_check_data<'a>(
    _config: &'a CheckConfig,
    graph: &'a Graph,
    db_root: &'a Path,
    opts: &CheckOptions,
    display_opts: &CheckDisplayOptions,
) -> Result<CheckData<'a>> {
    let local_link_kinds = display_opts.local_link_kinds();
    let link_jobs = graph.collect_local_link_check_jobs(&local_link_kinds);
    let mut link_results = run_local_link_checks(link_jobs, db_root);
    if opts.checks.requests(CheckItem::RemoteFileLinks) {
        let ssh_jobs = graph.collect_ssh_file_link_check_jobs();
        let ssh_options = SshFileCheckOptions::default();
        link_results.extend(run_ssh_link_checks(ssh_jobs, &ssh_options));
    }
    let (broken_file, file_link_errors, broken_attachment) = split_link_check_results(link_results);

    let validation_issues =
        graph.collect_validation_issues(db_root, &display_opts.validation_options());

    let cross_link_result = if let Some(pair) = &opts.cross_links {
        let node_a = graph.resolve_target(&pair.source)?;
        let node_b = graph.resolve_target(&pair.target)?;
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
            source_uuid: node_a.uuid.to_string(),
            source_title: node_a.title.clone(),
            target_uuid: node_b.uuid.to_string(),
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

    for broken in results.broken {
        match broken.target.kind {
            LinkCheckKind::File => broken_file.push(BrokenFileLinkEntry {
                source_uuid: broken.source.uuid,
                source_title: broken.source.title,
                target_path: broken.target.path,
            }),
            LinkCheckKind::Attachment => broken_attachment.push(BrokenAttachmentLinkEntry {
                source_uuid: broken.source.uuid,
                source_title: broken.source.title,
                target_path: broken.target.path,
            }),
        }
    }

    for error in results.errors {
        if error.target.kind == LinkCheckKind::File {
            file_link_errors.push(file_link_error_entry(error));
        }
    }

    (broken_file, file_link_errors, broken_attachment)
}

fn file_link_error_entry(error: LinkCheckErrorTarget) -> FileLinkErrorEntry {
    FileLinkErrorEntry {
        source_uuid: error.source.uuid,
        source_title: error.source.title,
        target_path: error.target.path,
        backend: error.backend.as_str().to_string(),
        error_kind: error.error_kind.as_str().to_string(),
        message: error.message,
    }
}

pub(super) struct CheckData<'a> {
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
        let show_id = opts.shows(CheckDisplaySection::IdLinks);
        let show_stats = opts.shows(CheckDisplaySection::Stats);
        let id_healthy = !show_id
            || (stats.broken_link_count == 0
                && stats.parse_error_count == 0
                && stats.duplicate_uuid_count == 0);
        let stats_healthy = !show_stats
            || (stats.broken_link_count == 0
                && stats.parse_error_count == 0
                && stats.duplicate_uuid_count == 0);
        id_healthy
            && stats_healthy
            && (!opts.shows(CheckDisplaySection::FileLinks)
                || (self.broken_file.is_empty() && self.file_link_errors.is_empty()))
            && (!opts.shows(CheckDisplaySection::AttachmentLinks)
                || self.broken_attachment.is_empty())
            && (!opts.shows(CheckDisplaySection::Filetags)
                || self.validation_issues.filetags.is_empty())
            && (!opts.shows(CheckDisplaySection::SelfLinks)
                || self.validation_issues.self_links.is_empty())
            && (!opts.shows(CheckDisplaySection::Overlinks)
                || self.validation_issues.overlinks.is_empty())
    }
}

pub(super) fn build_check_output(data: &CheckData, opts: &CheckDisplayOptions) -> CheckOutput {
    let stats = data.graph.stats();
    let show_id = opts.shows(CheckDisplaySection::IdLinks);

    let broken = if show_id {
        data.validation_issues
            .broken_internal_links
            .iter()
            .map(|issue| BrokenLinkEntry {
                source_uuid: issue.source_uuid.to_string(),
                source_title: issue.source_title.clone(),
                target_uuid: issue.target_uuid.to_string(),
            })
            .collect()
    } else {
        vec![]
    };

    let failed = if show_id {
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
        stats: if opts.shows(CheckDisplaySection::Stats) {
            Some(stats.clone())
        } else {
            None
        },
        duplicates: if show_id {
            data.validation_issues.duplicates.clone()
        } else {
            None
        },
        broken_links: if show_id { Some(broken) } else { None },
        broken_file_links: if opts.shows(CheckDisplaySection::FileLinks) {
            Some(data.broken_file.to_vec())
        } else {
            None
        },
        file_link_errors: if opts.shows(CheckDisplaySection::FileLinks) {
            Some(data.file_link_errors.to_vec())
        } else {
            None
        },
        broken_attachment_links: if opts.shows(CheckDisplaySection::AttachmentLinks) {
            Some(data.broken_attachment.to_vec())
        } else {
            None
        },
        failed_files: if show_id { Some(failed) } else { None },
        filetags_issues: if opts.shows(CheckDisplaySection::Filetags) {
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
        self_links: if opts.shows(CheckDisplaySection::SelfLinks) {
            Some(data.validation_issues.self_links.to_vec())
        } else {
            None
        },
        overlinks: if opts.shows(CheckDisplaySection::Overlinks) {
            Some(data.validation_issues.overlinks.to_vec())
        } else {
            None
        },
        cross_links: data.cross_link_result.clone(),
        healthy,
    }
}
