use super::{LinkCheckBackend, LinkCheckJob, LinkCheckKind, local_file_link_target_exists};
use crate::link_check::model::{
    LinkCheckErrorKind, LinkCheckOutcome, LinkCheckResults, link_check_broken, link_check_error,
};
use pkms_org::attachments::attachment_target_exists;
use rayon::prelude::*;
use std::path::Path;

pub fn check_local_link_job(job: LinkCheckJob, db_root: &Path) -> LinkCheckOutcome {
    let exists = match job.backend {
        LinkCheckBackend::Local => match job.target.kind {
            LinkCheckKind::File => {
                local_file_link_target_exists(job.target.as_str(), &job.source.path, db_root)
            }
            LinkCheckKind::Attachment => {
                attachment_target_exists(db_root, &job.source.uuid, job.target.as_str())
            }
        },
        LinkCheckBackend::Ssh => {
            return LinkCheckOutcome::Error(link_check_error(
                job,
                LinkCheckErrorKind::UnsupportedBackend,
                "SSH link jobs cannot be checked by the local checker",
            ));
        }
    };

    if exists {
        LinkCheckOutcome::Ok
    } else {
        LinkCheckOutcome::Broken(link_check_broken(job))
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
    results.sort();
    results
}
