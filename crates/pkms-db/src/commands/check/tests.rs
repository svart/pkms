use super::*;
use crate::link_check::LinkCheckKind;
use pkms_org::OrgConfig;
use pkms_org::graph::{Graph, GraphStats};

fn org_config(db_root: &std::path::Path) -> OrgConfig {
    OrgConfig {
        db_root: db_root.to_path_buf(),
        new_notes_dir: None,
        daily_notes_dir: None,
        ignore_patterns: Vec::new(),
    }
}

fn check_config(db_root: &std::path::Path) -> CheckConfig {
    CheckConfig {
        org: org_config(db_root),
    }
}

fn healthy_output() -> CheckOutput {
    CheckOutput {
        db_root: "/notes".to_string(),
        stats: Some(GraphStats {
            total_notes: 2,
            total_links: 3,
            total_internal_links: 1,
            total_file_links: 1,
            total_url_links: 1,
            total_ssh_links: 0,
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
    assert!(text.contains("  Links:          3"));
    assert!(text.contains("    internal: 1"));
    assert!(text.contains("    url: 1"));
    assert!(text.contains("    file: 1"));
    assert!(text.contains("    ssh: 0"));
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
    let db_config = check_config(dir.path());
    let output = execute(
        &db_config,
        &CheckOptions {
            checks: CheckSelection::Explicit(vec![CheckItem::FileLinks]),
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
    let graph = Graph::load(&org_config(db_root)).unwrap();

    let all_jobs =
        graph.collect_local_link_check_jobs(&[LinkCheckKind::File, LinkCheckKind::Attachment]);
    let all_observed: Vec<_> = all_jobs
        .iter()
        .map(|job| {
            (
                job.target.kind,
                job.source.uuid.as_str(),
                job.source.title.as_str(),
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

    let file_jobs = graph.collect_local_link_check_jobs(&[LinkCheckKind::File]);
    assert_eq!(file_jobs.len(), 2);
    assert!(
        file_jobs
            .iter()
            .all(|job| job.target.kind == LinkCheckKind::File)
    );

    let attachment_jobs = graph.collect_local_link_check_jobs(&[LinkCheckKind::Attachment]);
    assert_eq!(attachment_jobs.len(), 2);
    assert!(
        attachment_jobs
            .iter()
            .all(|job| job.target.kind == LinkCheckKind::Attachment)
    );
}
