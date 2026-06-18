use crate::graph::file_link_target_exists;
use crate::util::attachment_target_exists;
use rayon::prelude::*;
use std::cmp::Ordering;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SshConnectionKey {
    pub user: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFileTarget {
    pub connection: SshConnectionKey,
    pub path: String,
    pub line_spec: Option<String>,
    pub raw_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshFileTargetParseErrorKind {
    EmptyUser,
    EmptyHost,
    InvalidPort,
    MissingPath,
    UnsupportedSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFileTargetParseError {
    pub kind: SshFileTargetParseErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckJob {
    pub kind: LinkCheckKind,
    pub backend: LinkCheckBackend,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCheckOutcome {
    Ok,
    Broken(LinkCheckBrokenTarget),
    Error(LinkCheckErrorTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckBrokenTarget {
    pub kind: LinkCheckKind,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckErrorTarget {
    pub kind: LinkCheckKind,
    pub backend: LinkCheckBackend,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
    pub error_kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkCheckResults {
    pub broken: Vec<LinkCheckBrokenTarget>,
    pub errors: Vec<LinkCheckErrorTarget>,
}

impl LinkCheckJob {
    pub fn file(
        source_uuid: impl Into<String>,
        source_title: impl Into<String>,
        source_path: impl Into<PathBuf>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            kind: LinkCheckKind::File,
            backend: LinkCheckBackend::Local,
            source_uuid: source_uuid.into(),
            source_title: source_title.into(),
            source_path: source_path.into(),
            target: target.into(),
        }
    }

    pub fn attachment(
        source_uuid: impl Into<String>,
        source_title: impl Into<String>,
        source_path: impl Into<PathBuf>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            kind: LinkCheckKind::Attachment,
            backend: LinkCheckBackend::Local,
            source_uuid: source_uuid.into(),
            source_title: source_title.into(),
            source_path: source_path.into(),
            target: target.into(),
        }
    }
}

pub fn sort_link_check_jobs(jobs: &mut [LinkCheckJob]) {
    jobs.sort_by(compare_jobs);
}

pub fn sort_broken_targets(targets: &mut [LinkCheckBrokenTarget]) {
    targets.sort_by(compare_broken_targets);
}

pub fn sort_link_check_errors(errors: &mut [LinkCheckErrorTarget]) {
    errors.sort_by(compare_error_targets);
}

pub fn is_ssh_file_target(target: &str) -> bool {
    normalized_file_target(target).starts_with("/ssh:")
}

pub fn split_file_link_line_spec(target: &str) -> (&str, Option<&str>) {
    target
        .split_once("::")
        .map_or((target, None), |(path, line_spec)| (path, Some(line_spec)))
}

pub fn parse_ssh_file_target(
    target: &str,
    default_user: &str,
) -> Result<Option<SshFileTarget>, SshFileTargetParseError> {
    let normalized = normalized_file_target(target);
    if !normalized.starts_with("/ssh:") {
        return Ok(None);
    }

    let (without_line_spec, line_spec) = split_file_link_line_spec(normalized);
    let rest = without_line_spec.trim_start_matches("/ssh:");
    if rest.contains('|') {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::UnsupportedSyntax,
            "SSH file links do not support multi-hop TRAMP syntax",
        ));
    }

    let (login, path) = rest.split_once(':').ok_or_else(|| {
        ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file link is missing a remote path",
        )
    })?;
    if path.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file link is missing a remote path",
        ));
    }
    if !path.starts_with('/') {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file links must use absolute remote paths",
        ));
    }

    let (user, host_and_port) = if let Some((user, host_and_port)) = login.rsplit_once('@') {
        if user.is_empty() {
            return Err(ssh_parse_error(
                SshFileTargetParseErrorKind::EmptyUser,
                "SSH file link has an empty user",
            ));
        }
        (user, host_and_port)
    } else {
        (default_user, login)
    };
    if user.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::EmptyUser,
            "SSH file link needs a user or a non-empty default user",
        ));
    }

    let (host, port) = parse_host_and_port(host_and_port)?;
    Ok(Some(SshFileTarget {
        connection: SshConnectionKey {
            user: user.to_string(),
            host,
            port,
        },
        path: path.to_string(),
        line_spec: line_spec.map(str::to_string),
        raw_target: target.to_string(),
    }))
}

pub fn local_file_link_target_exists(target: &str, source_path: &Path, db_root: &Path) -> bool {
    if is_ssh_file_target(target) {
        return true;
    }
    file_link_target_exists(target, source_path, db_root)
}

pub fn check_local_link_job(job: LinkCheckJob, db_root: &Path) -> LinkCheckOutcome {
    let exists = match job.backend {
        LinkCheckBackend::Local => match job.kind {
            LinkCheckKind::File => {
                local_file_link_target_exists(&job.target, &job.source_path, db_root)
            }
            LinkCheckKind::Attachment => {
                attachment_target_exists(db_root, &job.source_uuid, &job.target)
            }
        },
        LinkCheckBackend::Ssh => {
            return LinkCheckOutcome::Error(LinkCheckErrorTarget {
                kind: job.kind,
                backend: job.backend,
                source_uuid: job.source_uuid,
                source_title: job.source_title,
                source_path: job.source_path,
                target: job.target,
                error_kind: "unsupported_backend".to_string(),
                message: "SSH link jobs cannot be checked by the local checker".to_string(),
            });
        }
    };

    if exists {
        LinkCheckOutcome::Ok
    } else {
        LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
            kind: job.kind,
            source_uuid: job.source_uuid,
            source_title: job.source_title,
            source_path: job.source_path,
            target: job.target,
        })
    }
}

pub fn run_local_link_checks(jobs: Vec<LinkCheckJob>, db_root: &Path) -> LinkCheckResults {
    let outcomes: Vec<LinkCheckOutcome> = jobs
        .into_par_iter()
        .map(|job| check_local_link_job(job, db_root))
        .collect();
    let mut results = LinkCheckResults::default();
    for outcome in outcomes {
        match outcome {
            LinkCheckOutcome::Ok => {}
            LinkCheckOutcome::Broken(target) => results.broken.push(target),
            LinkCheckOutcome::Error(target) => results.errors.push(target),
        }
    }
    sort_broken_targets(&mut results.broken);
    sort_link_check_errors(&mut results.errors);
    results
}

fn compare_jobs(a: &LinkCheckJob, b: &LinkCheckJob) -> Ordering {
    job_sort_key(a).cmp(&job_sort_key(b))
}

fn compare_broken_targets(a: &LinkCheckBrokenTarget, b: &LinkCheckBrokenTarget) -> Ordering {
    broken_target_sort_key(a).cmp(&broken_target_sort_key(b))
}

fn compare_error_targets(a: &LinkCheckErrorTarget, b: &LinkCheckErrorTarget) -> Ordering {
    error_target_sort_key(a).cmp(&error_target_sort_key(b))
}

fn job_sort_key(job: &LinkCheckJob) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        job.kind,
        job.source_uuid.as_str(),
        job.source_title.as_str(),
        job.source_path.as_path(),
        job.target.as_str(),
    )
}

fn broken_target_sort_key(
    target: &LinkCheckBrokenTarget,
) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        target.kind,
        target.source_uuid.as_str(),
        target.source_title.as_str(),
        target.source_path.as_path(),
        target.target.as_str(),
    )
}

fn error_target_sort_key(
    target: &LinkCheckErrorTarget,
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
        target.kind,
        target.backend,
        target.source_uuid.as_str(),
        target.source_title.as_str(),
        target.source_path.as_path(),
        target.target.as_str(),
        target.error_kind.as_str(),
    )
}

fn normalized_file_target(target: &str) -> &str {
    target.strip_prefix("org:").unwrap_or(target)
}

fn parse_host_and_port(host_and_port: &str) -> Result<(String, u16), SshFileTargetParseError> {
    let (host, port) = if let Some((host, raw_port)) = host_and_port.rsplit_once('#') {
        if raw_port.is_empty() {
            return Err(ssh_parse_error(
                SshFileTargetParseErrorKind::InvalidPort,
                "SSH file link has an empty port",
            ));
        }
        let port = raw_port.parse::<u16>().map_err(|_| {
            ssh_parse_error(
                SshFileTargetParseErrorKind::InvalidPort,
                "SSH file link has an invalid port",
            )
        })?;
        (host, port)
    } else {
        (host_and_port, 22)
    };

    if host.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::EmptyHost,
            "SSH file link has an empty host",
        ));
    }
    Ok((host.to_string(), port))
}

fn ssh_parse_error(
    kind: SshFileTargetParseErrorKind,
    message: impl Into<String>,
) -> SshFileTargetParseError {
    SshFileTargetParseError {
        kind,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_jobs_use_existing_line_spec_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");
        let target_path = db_root.join("target.org");
        std::fs::write(&source_path, "#+title: Source\n").unwrap();
        std::fs::write(&target_path, "first line\nneedle line\n").unwrap();

        let ok = check_local_link_job(
            LinkCheckJob::file("source", "Source", &source_path, "target.org::needle"),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::file("source", "Source", &source_path, "target.org::missing"),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                kind: LinkCheckKind::File,
                source_uuid: "source".to_string(),
                source_title: "Source".to_string(),
                source_path,
                target: "target.org::missing".to_string(),
            })
        );
    }

    #[test]
    fn attachment_jobs_use_hashed_org_attach_layout() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-bbbbbbbbbbbb";
        let attach_dir = db_root.join(".attach").join(&uuid[..2]).join(&uuid[2..]);
        std::fs::create_dir_all(&attach_dir).unwrap();
        std::fs::write(attach_dir.join("image.png"), b"image").unwrap();

        let ok = check_local_link_job(
            LinkCheckJob::attachment(uuid, "Source", &source_path, "image.png"),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::attachment(uuid, "Source", &source_path, "missing.png"),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                kind: LinkCheckKind::Attachment,
                source_uuid: uuid.to_string(),
                source_title: "Source".to_string(),
                source_path,
                target: "missing.png".to_string(),
            })
        );
    }

    #[test]
    fn parses_tramp_ssh_file_targets() {
        let target = parse_ssh_file_target(
            "/ssh:alice@example.org#2222:/var/log/app.log::needle",
            "local",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            target,
            SshFileTarget {
                connection: SshConnectionKey {
                    user: "alice".to_string(),
                    host: "example.org".to_string(),
                    port: 2222,
                },
                path: "/var/log/app.log".to_string(),
                line_spec: Some("needle".to_string()),
                raw_target: "/ssh:alice@example.org#2222:/var/log/app.log::needle".to_string(),
            }
        );
    }

    #[test]
    fn parses_ssh_file_targets_with_default_user_and_port() {
        let target = parse_ssh_file_target("org:/ssh:example.org:/tmp/file.txt", "local")
            .unwrap()
            .unwrap();

        assert_eq!(target.connection.user, "local");
        assert_eq!(target.connection.host, "example.org");
        assert_eq!(target.connection.port, 22);
        assert_eq!(target.path, "/tmp/file.txt");
        assert_eq!(target.line_spec, None);
    }

    #[test]
    fn rejects_unsupported_ssh_file_target_syntax() {
        let error =
            parse_ssh_file_target("/ssh:jump|example.org:/tmp/file.txt", "local").unwrap_err();

        assert_eq!(error.kind, SshFileTargetParseErrorKind::UnsupportedSyntax);
    }

    #[test]
    fn local_file_checks_skip_ssh_file_targets() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");

        assert!(local_file_link_target_exists(
            "/ssh:example.org:/missing/file.txt::needle",
            &source_path,
            db_root
        ));
    }
}
