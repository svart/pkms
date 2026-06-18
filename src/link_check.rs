use crate::graph::file_link_target_exists;
use crate::util::attachment_target_exists;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCheckKind {
    File,
    Attachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCheckBackend {
    Local,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckBrokenTarget {
    pub kind: LinkCheckKind,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
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

pub fn local_file_link_target_exists(target: &str, source_path: &Path, db_root: &Path) -> bool {
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

pub fn run_local_link_checks(
    jobs: Vec<LinkCheckJob>,
    db_root: &Path,
) -> Vec<LinkCheckBrokenTarget> {
    let mut broken: Vec<LinkCheckBrokenTarget> = jobs
        .into_par_iter()
        .filter_map(|job| match check_local_link_job(job, db_root) {
            LinkCheckOutcome::Ok => None,
            LinkCheckOutcome::Broken(target) => Some(target),
        })
        .collect();
    sort_broken_targets(&mut broken);
    broken
}

fn compare_jobs(a: &LinkCheckJob, b: &LinkCheckJob) -> Ordering {
    compare_job_parts(
        a.kind,
        &a.source_uuid,
        &a.source_title,
        &a.source_path,
        &a.target,
        b.kind,
        &b.source_uuid,
        &b.source_title,
        &b.source_path,
        &b.target,
    )
}

fn compare_broken_targets(a: &LinkCheckBrokenTarget, b: &LinkCheckBrokenTarget) -> Ordering {
    compare_job_parts(
        a.kind,
        &a.source_uuid,
        &a.source_title,
        &a.source_path,
        &a.target,
        b.kind,
        &b.source_uuid,
        &b.source_title,
        &b.source_path,
        &b.target,
    )
}

#[allow(clippy::too_many_arguments)]
fn compare_job_parts(
    a_kind: LinkCheckKind,
    a_uuid: &str,
    a_title: &str,
    a_path: &Path,
    a_target: &str,
    b_kind: LinkCheckKind,
    b_uuid: &str,
    b_title: &str,
    b_path: &Path,
    b_target: &str,
) -> Ordering {
    kind_order(a_kind)
        .cmp(&kind_order(b_kind))
        .then_with(|| a_uuid.cmp(b_uuid))
        .then_with(|| a_title.cmp(b_title))
        .then_with(|| a_path.cmp(b_path))
        .then_with(|| a_target.cmp(b_target))
}

fn kind_order(kind: LinkCheckKind) -> u8 {
    match kind {
        LinkCheckKind::File => 0,
        LinkCheckKind::Attachment => 1,
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
}
