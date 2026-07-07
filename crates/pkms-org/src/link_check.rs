use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkCheckKind {
    File,
    Attachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkCheckBackend {
    Local,
    Ssh,
}

impl LinkCheckBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkCheckBackend::Local => "local",
            LinkCheckBackend::Ssh => "ssh",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSource {
    pub uuid: String,
    pub title: String,
    pub path: PathBuf,
}

impl LinkSource {
    pub fn new(
        uuid: impl Into<String>,
        title: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            uuid: uuid.into(),
            title: title.into(),
            path: path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckTarget {
    pub kind: LinkCheckKind,
    pub path: String,
}

impl LinkCheckTarget {
    pub fn new(kind: LinkCheckKind, path: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.path.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckJob {
    pub backend: LinkCheckBackend,
    pub source: LinkSource,
    pub target: LinkCheckTarget,
}

impl LinkCheckJob {
    pub fn new(
        kind: LinkCheckKind,
        backend: LinkCheckBackend,
        source: LinkSource,
        target: impl Into<String>,
    ) -> Self {
        Self {
            backend,
            source,
            target: LinkCheckTarget::new(kind, target),
        }
    }
}

pub fn sort_link_check_jobs(jobs: &mut [LinkCheckJob]) {
    jobs.sort_by(|a, b| job_sort_key(a).cmp(&job_sort_key(b)));
}

pub fn is_ssh_file_target(target: &str) -> bool {
    normalized_file_target(target).starts_with("/ssh:")
}

pub fn split_file_link_line_spec(target: &str) -> (&str, Option<&str>) {
    target
        .split_once("::")
        .map_or((target, None), |(path, line_spec)| (path, Some(line_spec)))
}

pub fn local_file_link_target_exists(target: &str, source_path: &Path, db_root: &Path) -> bool {
    local_file_link_target_exists_with_home(target, source_path, db_root, None)
}

pub fn local_file_link_target_exists_with_home(
    target: &str,
    source_path: &Path,
    db_root: &Path,
    home_dir: Option<&Path>,
) -> bool {
    if is_ssh_file_target(target) {
        return true;
    }
    crate::graph::file_link_target_exists_with_home(target, source_path, db_root, home_dir)
}

fn normalized_file_target(target: &str) -> &str {
    target.strip_prefix("org:").unwrap_or(target)
}

fn job_sort_key(job: &LinkCheckJob) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        job.target.kind,
        job.source.uuid.as_str(),
        job.source.title.as_str(),
        job.source.path.as_path(),
        job.target.as_str(),
    )
}
