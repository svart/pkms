mod local;
mod model;
mod ssh;

pub use pkms_org::link_check::{
    LinkCheckBackend, LinkCheckJob, LinkCheckKind, LinkCheckTarget, LinkSource, is_ssh_file_target,
    local_file_link_target_exists, local_file_link_target_exists_with_home, sort_link_check_jobs,
    split_file_link_line_spec,
};

pub use local::{
    check_local_link_job, check_local_link_job_with_home, run_local_link_checks,
    run_local_link_checks_with_home,
};
pub use model::{
    LinkCheckBrokenTarget, LinkCheckErrorKind, LinkCheckErrorTarget, LinkCheckOutcome,
    LinkCheckResults, SshErrorKind, sort_broken_targets, sort_link_check_errors,
};
pub use ssh::{
    SshConnectionKey, SshFileCheckOptions, SshFileTarget, SshFileTargetParseError,
    SshFileTargetParseErrorKind, parse_ssh_file_target, run_ssh_link_checks,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_check_error_kind_strings_match_output_contract() {
        assert_eq!(LinkCheckErrorKind::Ssh(SshErrorKind::Auth).as_str(), "auth");
        assert_eq!(
            LinkCheckErrorKind::Ssh(SshErrorKind::HostKey).as_str(),
            "hostkey"
        );
        assert_eq!(
            LinkCheckErrorKind::Ssh(SshErrorKind::Network).as_str(),
            "network"
        );
        assert_eq!(
            LinkCheckErrorKind::Ssh(SshErrorKind::Timeout).as_str(),
            "timeout"
        );
        assert_eq!(LinkCheckErrorKind::Unsupported.as_str(), "unsupported");
        assert_eq!(
            LinkCheckErrorKind::UnsupportedBackend.as_str(),
            "unsupported_backend"
        );
    }

    #[test]
    fn file_jobs_use_existing_line_spec_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");
        let target_path = db_root.join("target.org");
        std::fs::write(&source_path, "#+title: Source\n").unwrap();
        std::fs::write(&target_path, "first line\nneedle line\n").unwrap();

        let ok = check_local_link_job(
            LinkCheckJob::new(
                LinkCheckKind::File,
                LinkCheckBackend::Local,
                LinkSource::new("source", "Source", source_path.clone()),
                "target.org::needle",
            ),
            db_root,
        );
        let line_number_ok = check_local_link_job(
            LinkCheckJob::new(
                LinkCheckKind::File,
                LinkCheckBackend::Local,
                LinkSource::new("source", "Source", source_path.clone()),
                "target.org::2",
            ),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::new(
                LinkCheckKind::File,
                LinkCheckBackend::Local,
                LinkSource::new("source", "Source", source_path.clone()),
                "target.org::missing",
            ),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(line_number_ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                source: LinkSource::new("source", "Source", source_path),
                target: LinkCheckTarget::new(LinkCheckKind::File, "target.org::missing"),
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
            LinkCheckJob::new(
                LinkCheckKind::Attachment,
                LinkCheckBackend::Local,
                LinkSource::new(uuid, "Source", source_path.clone()),
                "image.png",
            ),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::new(
                LinkCheckKind::Attachment,
                LinkCheckBackend::Local,
                LinkSource::new(uuid, "Source", source_path.clone()),
                "missing.png",
            ),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                source: LinkSource::new(uuid, "Source", source_path),
                target: LinkCheckTarget::new(LinkCheckKind::Attachment, "missing.png"),
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
