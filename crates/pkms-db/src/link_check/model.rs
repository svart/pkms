use super::{LinkCheckBackend, LinkCheckJob, LinkCheckKind, LinkCheckTarget, LinkSource};
use std::cmp::Ordering;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    Auth,
    HostKey,
    Network,
    Timeout,
}

impl SshErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SshErrorKind::Auth => "auth",
            SshErrorKind::HostKey => "hostkey",
            SshErrorKind::Network => "network",
            SshErrorKind::Timeout => "timeout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCheckErrorKind {
    Ssh(SshErrorKind),
    Unsupported,
    UnsupportedBackend,
}

impl LinkCheckErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkCheckErrorKind::Ssh(kind) => kind.as_str(),
            LinkCheckErrorKind::Unsupported => "unsupported",
            LinkCheckErrorKind::UnsupportedBackend => "unsupported_backend",
        }
    }
}

impl From<SshErrorKind> for LinkCheckErrorKind {
    fn from(kind: SshErrorKind) -> Self {
        LinkCheckErrorKind::Ssh(kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCheckOutcome {
    Ok,
    Broken(LinkCheckBrokenTarget),
    Error(LinkCheckErrorTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckBrokenTarget {
    pub source: LinkSource,
    pub target: LinkCheckTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckErrorTarget {
    pub backend: LinkCheckBackend,
    pub source: LinkSource,
    pub target: LinkCheckTarget,
    pub error_kind: LinkCheckErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkCheckResults {
    pub broken: Vec<LinkCheckBrokenTarget>,
    pub errors: Vec<LinkCheckErrorTarget>,
}

impl LinkCheckResults {
    pub fn extend(&mut self, mut other: Self) {
        self.broken.append(&mut other.broken);
        self.errors.append(&mut other.errors);
        self.sort();
    }

    pub fn sort(&mut self) {
        sort_broken_targets(&mut self.broken);
        sort_link_check_errors(&mut self.errors);
    }
}

pub fn sort_broken_targets(targets: &mut [LinkCheckBrokenTarget]) {
    targets.sort_by(compare_broken_targets);
}

pub fn sort_link_check_errors(errors: &mut [LinkCheckErrorTarget]) {
    errors.sort_by(compare_error_targets);
}

fn compare_broken_targets(a: &LinkCheckBrokenTarget, b: &LinkCheckBrokenTarget) -> Ordering {
    broken_target_sort_key(a).cmp(&broken_target_sort_key(b))
}

fn compare_error_targets(a: &LinkCheckErrorTarget, b: &LinkCheckErrorTarget) -> Ordering {
    error_target_sort_key(a).cmp(&error_target_sort_key(b))
}

fn broken_target_sort_key(
    broken: &LinkCheckBrokenTarget,
) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        broken.target.kind,
        broken.source.uuid.as_str(),
        broken.source.title.as_str(),
        broken.source.path.as_path(),
        broken.target.as_str(),
    )
}

fn error_target_sort_key(
    error: &LinkCheckErrorTarget,
) -> (
    LinkCheckKind,
    LinkCheckBackend,
    &str,
    &str,
    &Path,
    &str,
    &str,
) {
    (
        error.target.kind,
        error.backend,
        error.source.uuid.as_str(),
        error.source.title.as_str(),
        error.source.path.as_path(),
        error.target.as_str(),
        error.error_kind.as_str(),
    )
}

pub(super) fn link_check_error(
    job: LinkCheckJob,
    error_kind: impl Into<LinkCheckErrorKind>,
    message: impl Into<String>,
) -> LinkCheckErrorTarget {
    LinkCheckErrorTarget {
        backend: job.backend,
        source: job.source,
        target: job.target,
        error_kind: error_kind.into(),
        message: message.into(),
    }
}

pub(super) fn link_check_broken(job: LinkCheckJob) -> LinkCheckBrokenTarget {
    LinkCheckBrokenTarget {
        source: job.source,
        target: job.target,
    }
}
