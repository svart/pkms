use pkms_org::graph::{DuplicateInfo, GraphStats, OverlinkEntry, SelfLinkEntry};
use serde::Serialize;

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
    pub checks: CheckSelection,
    pub cross_links: Option<CrossLinkTargets>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckSelection {
    Default,
    Explicit(Vec<CheckItem>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckItem {
    Stats,
    FileLinks,
    RemoteFileLinks,
    AttachmentLinks,
    IdLinks,
    Filetags,
    SelfLinks,
    Overlinks,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossLinkTargets {
    pub source: String,
    pub target: String,
}

impl CheckSelection {
    pub(super) fn requests(&self, check: CheckItem) -> bool {
        match self {
            CheckSelection::Default => false,
            CheckSelection::Explicit(checks) => checks.contains(&check),
        }
    }
}
