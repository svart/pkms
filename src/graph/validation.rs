use super::{DuplicateInfo, Graph, Node, OverlinkEntry, SelfLinkEntry, resolve_file_link_path};
use crate::link_check::{
    LinkCheckJob, LinkCheckKind, is_ssh_file_target, local_file_link_target_exists,
    sort_link_check_jobs,
};
use crate::parser::{ID_PROPERTY_RE, Link, TITLE_RE, UUID_FORMAT_RE, validate_filetags_format};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct GraphValidationOptions {
    checks: Vec<GraphValidationCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphValidationCheck {
    InternalLinks,
    Filetags,
    Duplicates,
    SelfLinks,
    Overlinks,
}

impl GraphValidationOptions {
    pub fn new(checks: impl IntoIterator<Item = GraphValidationCheck>) -> Self {
        GraphValidationOptions {
            checks: checks.into_iter().collect(),
        }
    }

    pub fn includes(&self, check: GraphValidationCheck) -> bool {
        self.checks.contains(&check)
    }
}

#[derive(Debug, Clone, Default)]
pub struct GraphValidationIssues {
    pub broken_internal_links: Vec<BrokenInternalLinkIssue>,
    pub filetags: Vec<FiletagsValidationIssue>,
    pub duplicates: Option<DuplicateInfo>,
    pub self_links: Vec<SelfLinkEntry>,
    pub overlinks: Vec<OverlinkEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct NodeValidationIssues {
    pub issues: Vec<NoteValidationIssue>,
    pub broken_internal_links: Vec<BrokenInternalLinkIssue>,
    pub broken_file_links: Vec<BrokenFileLinkIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenInternalLinkIssue {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenFileLinkIssue {
    pub source_uuid: String,
    pub source_title: String,
    pub target_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiletagsValidationIssue {
    pub path: PathBuf,
    pub title: String,
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteValidationIssue {
    InvalidUuidFormat {
        uuid: String,
    },
    MissingTitle,
    InvalidFiletagsFormat {
        raw: String,
        reason: String,
    },
    DuplicateUuid {
        uuid: String,
        kind: DuplicateUuidIssueKind,
    },
    SelfLink {
        link_type: SelfLinkKind,
        target: String,
        suggested_uuid: Option<String>,
    },
    Overlink {
        target_uuid: String,
        target_title: String,
        count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateUuidIssueKind {
    HeadingMatchesPrimary,
    HeadingRepeatedInNote,
    HeadingBelongsToAnotherNote { other_paths: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfLinkKind {
    Id,
    File,
}

impl Graph {
    pub fn collect_validation_issues(
        &self,
        db_root: &Path,
        options: &GraphValidationOptions,
    ) -> GraphValidationIssues {
        GraphValidationIssues {
            broken_internal_links: if options.includes(GraphValidationCheck::InternalLinks) {
                self.collect_broken_internal_link_issues()
            } else {
                Vec::new()
            },
            filetags: if options.includes(GraphValidationCheck::Filetags) {
                self.collect_filetags_issues()
            } else {
                Vec::new()
            },
            duplicates: options
                .includes(GraphValidationCheck::Duplicates)
                .then(|| self.duplicates.clone()),
            self_links: if options.includes(GraphValidationCheck::SelfLinks) {
                self.detect_self_links(db_root)
            } else {
                Vec::new()
            },
            overlinks: if options.includes(GraphValidationCheck::Overlinks) {
                self.detect_overlinks()
            } else {
                Vec::new()
            },
        }
    }

    pub fn collect_node_validation_issues(
        &self,
        node: &Node,
        target: &str,
        db_root: &Path,
    ) -> NodeValidationIssues {
        let content = self.raw_content_for_path(&node.path).unwrap_or_default();
        let mut issues = Vec::new();

        collect_uuid_format_issue(node, &mut issues);
        collect_title_presence_issue(content, &mut issues);
        collect_filetags_format_issues(content, &mut issues);
        collect_duplicate_uuid_issues(content, self, node, &mut issues);
        collect_node_self_link_issues(self, node, target, db_root, &mut issues);
        collect_node_overlink_issues(self, node, &mut issues);

        NodeValidationIssues {
            issues,
            broken_internal_links: self.collect_node_broken_internal_link_issues(node),
            broken_file_links: collect_node_broken_file_link_issues(node, db_root),
        }
    }

    pub fn collect_local_link_check_jobs(&self, kinds: &[LinkCheckKind]) -> Vec<LinkCheckJob> {
        let mut jobs = Vec::new();
        for node in self.nodes.values() {
            for link in &node.outgoing {
                match link {
                    Link::File(target)
                        if kinds.contains(&LinkCheckKind::File) && !is_ssh_file_target(target) =>
                    {
                        jobs.push(LinkCheckJob::file(
                            node.uuid.clone(),
                            node.title.clone(),
                            node.path.clone(),
                            target.clone(),
                        ));
                    }
                    Link::Attachment(target) if kinds.contains(&LinkCheckKind::Attachment) => {
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

    pub fn collect_ssh_file_link_check_jobs(&self) -> Vec<LinkCheckJob> {
        let mut jobs = Vec::new();
        for node in self.nodes.values() {
            for link in &node.outgoing {
                if let Link::File(target) = link
                    && is_ssh_file_target(target)
                {
                    jobs.push(LinkCheckJob::ssh_file(
                        node.uuid.clone(),
                        node.title.clone(),
                        node.path.clone(),
                        target.clone(),
                    ));
                }
            }
        }
        sort_link_check_jobs(&mut jobs);
        jobs
    }

    fn collect_broken_internal_link_issues(&self) -> Vec<BrokenInternalLinkIssue> {
        self.broken_links
            .iter()
            .map(|(source_uuid, target_uuid)| BrokenInternalLinkIssue {
                source_uuid: source_uuid.clone(),
                source_title: self
                    .nodes
                    .get(source_uuid)
                    .map(|node| node.title.clone())
                    .unwrap_or_default(),
                target_uuid: target_uuid.clone(),
            })
            .collect()
    }

    fn collect_node_broken_internal_link_issues(
        &self,
        node: &Node,
    ) -> Vec<BrokenInternalLinkIssue> {
        node.outgoing
            .iter()
            .filter_map(|link| match link {
                Link::Internal(uuid) if !self.nodes.contains_key(uuid) => {
                    Some(BrokenInternalLinkIssue {
                        source_uuid: node.uuid.clone(),
                        source_title: node.title.clone(),
                        target_uuid: uuid.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }

    fn collect_filetags_issues(&self) -> Vec<FiletagsValidationIssue> {
        let mut issues = Vec::new();
        for result in &self.results {
            if let Some(content) = result.raw_content.as_deref() {
                let node = self
                    .path_to_uuid
                    .get(&result.path)
                    .and_then(|uuid| self.nodes.get(uuid));
                issues.extend(filetags_issues_for_content(
                    content,
                    result.path.clone(),
                    node.map(|node| node.title.clone())
                        .or_else(|| result.parsed.title.clone())
                        .unwrap_or_default(),
                ));
            }
        }
        issues
    }

    pub fn detect_self_links(&self, db_root: &Path) -> Vec<SelfLinkEntry> {
        let mut results = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        for path in self.path_to_uuid.keys() {
            if !seen_paths.insert(path.clone()) {
                continue;
            }
            let Some(primary_uuid) = self.path_to_uuid.get(path) else {
                continue;
            };

            let has_headings = self
                .nodes
                .values()
                .any(|n| n.path == *path && n.uuid != *primary_uuid);

            for node in self.nodes.values() {
                if node.path != *path {
                    continue;
                }
                for link in &node.outgoing {
                    match link {
                        Link::Internal(uuid) if uuid == &node.uuid => {
                            results.push(SelfLinkEntry {
                                source_uuid: node.uuid.clone(),
                                source_title: node.title.clone(),
                                link_type: "id".to_string(),
                                target: uuid.clone(),
                                suggestion: None,
                            });
                        }
                        Link::File(target_path) => {
                            let resolved = resolve_file_link_path(target_path, &node.path, db_root);
                            if resolved == *path {
                                let suggestion = if has_headings {
                                    Some(format!(
                                        "Use id:{} instead of file link to this file",
                                        primary_uuid
                                    ))
                                } else {
                                    None
                                };
                                results.push(SelfLinkEntry {
                                    source_uuid: node.uuid.clone(),
                                    source_title: node.title.clone(),
                                    link_type: "file".to_string(),
                                    target: target_path.clone(),
                                    suggestion,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        results
    }

    pub fn detect_overlinks(&self) -> Vec<OverlinkEntry> {
        let mut results = Vec::new();
        for node in self.nodes.values() {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for link in &node.outgoing {
                if let Link::Internal(target) = link {
                    *counts.entry(target.clone()).or_default() += 1;
                }
            }
            for (target_uuid, count) in counts {
                if count >= 2 {
                    let target_title = self
                        .nodes
                        .get(&target_uuid)
                        .map(|n| n.title.clone())
                        .unwrap_or_default();
                    results.push(OverlinkEntry {
                        source_uuid: node.uuid.clone(),
                        source_title: node.title.clone(),
                        target_uuid,
                        target_title,
                        count,
                    });
                }
            }
        }
        results
    }
}

fn collect_uuid_format_issue(node: &Node, issues: &mut Vec<NoteValidationIssue>) {
    let uuid_parts: Vec<&str> = node.uuid.split('-').collect();
    if uuid_parts.len() != 5 {
        issues.push(NoteValidationIssue::InvalidUuidFormat {
            uuid: node.uuid.clone(),
        });
    }
}

fn collect_title_presence_issue(content: &str, issues: &mut Vec<NoteValidationIssue>) {
    if !TITLE_RE.is_match(content) {
        issues.push(NoteValidationIssue::MissingTitle);
    }
}

fn collect_filetags_format_issues(content: &str, issues: &mut Vec<NoteValidationIssue>) {
    issues.extend(
        validate_filetags_format(content)
            .into_iter()
            .map(|(raw, reason)| NoteValidationIssue::InvalidFiletagsFormat { raw, reason }),
    );
}

fn collect_duplicate_uuid_issues(
    content: &str,
    graph: &Graph,
    node: &Node,
    issues: &mut Vec<NoteValidationIssue>,
) {
    let all_ids: Vec<String> = ID_PROPERTY_RE
        .captures_iter(content)
        .filter_map(|captures| captures.get(1))
        .map(|matched| matched.as_str().to_string())
        .collect();
    if all_ids.len() <= 1 {
        return;
    }

    let primary = &all_ids[0];
    let mut seen_heading_ids = HashSet::new();
    let current_path = node.path.display().to_string();
    for id in all_ids.iter().skip(1) {
        if id == primary {
            issues.push(NoteValidationIssue::DuplicateUuid {
                uuid: id.clone(),
                kind: DuplicateUuidIssueKind::HeadingMatchesPrimary,
            });
        } else if !seen_heading_ids.insert(id.clone()) {
            issues.push(NoteValidationIssue::DuplicateUuid {
                uuid: id.clone(),
                kind: DuplicateUuidIssueKind::HeadingRepeatedInNote,
            });
        } else if let Some(duplicate) = graph.duplicates.duplicate_uuids.iter().find(|entry| {
            entry.value == *id && entry.paths.iter().any(|path| path != &current_path)
        }) {
            let other_paths = duplicate
                .paths
                .iter()
                .filter(|path| *path != &current_path)
                .cloned()
                .collect::<Vec<_>>();
            issues.push(NoteValidationIssue::DuplicateUuid {
                uuid: id.clone(),
                kind: DuplicateUuidIssueKind::HeadingBelongsToAnotherNote { other_paths },
            });
        }
    }
}

fn collect_node_broken_file_link_issues(node: &Node, db_root: &Path) -> Vec<BrokenFileLinkIssue> {
    node.outgoing
        .iter()
        .filter_map(|link| match link {
            Link::File(path_str)
                if !local_file_link_target_exists(path_str, &node.path, db_root) =>
            {
                Some(BrokenFileLinkIssue {
                    source_uuid: node.uuid.clone(),
                    source_title: node.title.clone(),
                    target_path: path_str.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

fn collect_node_self_link_issues(
    graph: &Graph,
    node: &Node,
    target: &str,
    db_root: &Path,
    issues: &mut Vec<NoteValidationIssue>,
) {
    let target_is_uuid = UUID_FORMAT_RE.is_match(target);
    let is_heading_node = graph.heading_uuid_to_primary.contains_key(&node.uuid);

    for link in &node.outgoing {
        match link {
            Link::Internal(uuid) if uuid == target || (!target_is_uuid && uuid == &node.uuid) => {
                issues.push(NoteValidationIssue::SelfLink {
                    link_type: SelfLinkKind::Id,
                    target: uuid.clone(),
                    suggested_uuid: None,
                });
            }
            Link::File(path) => {
                let resolved = resolve_file_link_path(path, &node.path, db_root);
                if resolved == node.path {
                    let suggested_uuid = if is_heading_node {
                        Some(
                            graph
                                .heading_uuid_to_primary
                                .get(&node.uuid)
                                .cloned()
                                .unwrap_or_else(|| node.uuid.clone()),
                        )
                    } else if target != node.uuid && target_is_uuid {
                        Some(node.uuid.clone())
                    } else {
                        None
                    };
                    issues.push(NoteValidationIssue::SelfLink {
                        link_type: SelfLinkKind::File,
                        target: path.clone(),
                        suggested_uuid,
                    });
                }
            }
            _ => {}
        }
    }
}

fn collect_node_overlink_issues(graph: &Graph, node: &Node, issues: &mut Vec<NoteValidationIssue>) {
    let mut target_counts: HashMap<String, usize> = HashMap::new();
    for link in &node.outgoing {
        if let Link::Internal(uuid) = link {
            *target_counts.entry(uuid.clone()).or_default() += 1;
        }
    }
    for (target_uuid, count) in target_counts {
        if count >= 2 {
            let target_title = graph
                .nodes
                .get(&target_uuid)
                .map(|node| node.title.clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            issues.push(NoteValidationIssue::Overlink {
                target_uuid,
                target_title,
                count,
            });
        }
    }
}

fn filetags_issues_for_content(
    content: &str,
    path: PathBuf,
    title: String,
) -> Vec<FiletagsValidationIssue> {
    validate_filetags_format(content)
        .into_iter()
        .map(|(raw, reason)| FiletagsValidationIssue {
            path: path.clone(),
            title: title.clone(),
            raw,
            reason,
        })
        .collect()
}
