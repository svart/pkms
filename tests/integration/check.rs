use super::*;

#[test]
fn test_check_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check"]);
    assert!(!status.success());
    assert!(stdout.contains("Notes:"), "stdout: {}", stdout);
    assert!(stdout.contains("Broken Note"), "stdout: {}", stdout);
}

#[test]
fn test_check_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (v, status) = run_json(&["--db", db, "--output-format", "json", "check"]);
    assert!(!status.success());
    assert!(v.get("stats").is_some());
    assert_eq!(v["healthy"], false);
    assert!(v.get("broken_links").is_some());
    assert!(
        v.get("filetags_issues").is_some(),
        "expected filetags_issues field"
    );
    let ft_count = v["filetags_issues"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        ft_count >= 1,
        "expected at least 1 filetags issue, got {}",
        ft_count
    );
    let dups = v["duplicates"]
        .get("duplicate_uuids")
        .and_then(|a| a.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(dups >= 1, "expected duplicate UUIDs");

    let (orphans, status) = run_json(&["--db", db, "--output-format", "json", "orphans"]);
    assert!(status.success());
    assert_eq!(v["stats"]["orphan_notes"], orphans["count"]);
}

#[test]
fn test_check_filetags_reports_file_once_when_heading_ids_exist() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "bad-filetags-heading.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Bad Filetags Heading
#+filetags: bad

* Heading
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--filetags",
    ]);
    assert!(!status.success(), "check should report bad filetags: {v}");
    assert_eq!(v["filetags_issues"].as_array().unwrap().len(), 1);
}

#[test]
fn test_check_ignores_missing_agenda_filetag() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "planned-task.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Planned Task

* TODO Planned task
SCHEDULED: <2026-06-04 Thu>
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
    ]);
    assert!(
        status.success(),
        "check should ignore missing agenda filetags: {v}"
    );
    assert!(
        v.get("agenda_issues").is_none(),
        "agenda_issues should not be emitted"
    );
}

#[test]
fn test_check_file_links_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check", "--file-links"]);
    assert!(!status.success());
    assert!(stdout.contains("Broken files:"));
    assert!(stdout.contains("File Link Note"));
}

#[test]
fn test_check_help_shows_remote_file_links_flag() {
    let (stdout, _stderr, status) = run(&["check", "--help"]);
    assert!(status.success());
    assert!(stdout.contains("--remote-file-links"), "stdout: {}", stdout);
}

#[test]
#[cfg(not(feature = "ssh"))]
fn test_check_remote_file_links_requires_ssh_feature() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "remote.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Remote

[[file:/ssh:example.com:/tmp/file.txt]]
"#,
    );

    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "check",
        "--remote-file-links",
    ]);
    assert!(!status.success());
    assert!(stdout.is_empty(), "stdout: {}", stdout);
    assert!(
        stderr.contains(
            "SSH file-link checks are not available in this build. Rebuild with --features ssh."
        ),
        "stderr: {}",
        stderr
    );
}

#[test]
#[cfg(feature = "ssh")]
fn test_check_remote_file_links_reports_unsupported_targets() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "remote.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Remote

[[file:/ssh:jump|example.com:/tmp/file.txt]]
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--remote-file-links",
    ]);
    assert!(
        !status.success(),
        "remote target should be unsupported: {v}"
    );
    assert_eq!(v["healthy"], false);
    assert_eq!(v["broken_file_links"].as_array().unwrap().len(), 0);
    let errors = v["file_link_errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["backend"], "ssh");
    assert_eq!(errors[0]["error_kind"], "unsupported");
    assert!(
        errors[0]["message"]
            .as_str()
            .unwrap()
            .contains("multi-hop TRAMP syntax")
    );
}

#[test]
#[ignore = "requires PKMS_TEST_SSH_TARGET, PKMS_TEST_SSH_PATH, and PKMS_TEST_SSH_MISSING_PATH"]
#[cfg(feature = "ssh")]
fn test_check_remote_file_links_live_ssh() {
    let target = std::env::var("PKMS_TEST_SSH_TARGET")
        .expect("set PKMS_TEST_SSH_TARGET to user@host or user@host#port");
    let existing_path = std::env::var("PKMS_TEST_SSH_PATH")
        .expect("set PKMS_TEST_SSH_PATH to an existing absolute remote path");
    let missing_path = std::env::var("PKMS_TEST_SSH_MISSING_PATH")
        .expect("set PKMS_TEST_SSH_MISSING_PATH to a missing absolute remote path");
    assert!(
        existing_path.starts_with('/'),
        "PKMS_TEST_SSH_PATH must be absolute"
    );
    assert!(
        missing_path.starts_with('/'),
        "PKMS_TEST_SSH_MISSING_PATH must be absolute"
    );

    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "remote.org",
        &format!(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Remote

[[file:/ssh:{target}:{existing_path}]]
[[file:/ssh:{target}:{missing_path}]]
"#,
        ),
    );

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "check",
            "--remote-file-links",
        ],
        &live_ssh_config(),
    );
    let v = assert_json_output(&["check", "--remote-file-links"], &stdout);

    assert!(
        !status.success(),
        "missing remote path should make check unhealthy\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert_eq!(v["healthy"], false);
    let errors = v["file_link_errors"].as_array().unwrap();
    assert!(errors.is_empty(), "unexpected SSH errors: {errors:?}");
    let broken = v["broken_file_links"].as_array().unwrap();
    assert_eq!(broken.len(), 1, "expected only the missing path: {v}");
    assert_eq!(
        broken[0]["target_path"],
        format!("/ssh:{target}:{missing_path}")
    );
}

#[cfg(feature = "ssh")]
fn live_ssh_config() -> String {
    let mut config = String::from("[ssh]\n");
    if let Ok(identity_file) = std::env::var("PKMS_TEST_SSH_IDENTITY") {
        config.push_str(&format!(
            "identity_file = {}\n",
            toml_string(&identity_file)
        ));
    }
    if let Ok(known_hosts) = std::env::var("PKMS_TEST_SSH_KNOWN_HOSTS") {
        config.push_str(&format!("known_hosts = {}\n", toml_string(&known_hosts)));
    }
    config
}

#[cfg(feature = "ssh")]
fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

#[test]
fn test_check_file_links_skips_ssh_targets_without_remote_flag() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "remote.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Remote

[[file:/ssh:example.com:/tmp/missing.txt::needle]]
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
    ]);

    assert!(status.success(), "check should skip remote targets: {v}");
    assert_eq!(v["healthy"], true);
    assert_eq!(v["broken_file_links"].as_array().unwrap().len(), 0);
}

#[test]
fn test_check_file_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
    ]);
    assert!(!status.success());
    assert!(
        v.get("broken_file_links").is_some(),
        "expected broken_file_links field"
    );
    let broken_files = v["broken_file_links"].as_array().unwrap();
    assert!(!broken_files.is_empty(), "expected broken file links");
    assert!(broken_files[0]["source_title"].is_string());
    assert!(broken_files[0]["target_path"].is_string());
    assert!(
        v.get("stats").is_none(),
        "stats should not appear with --file-links only"
    );
    assert!(
        v.get("broken_links").is_none(),
        "broken_links should not appear with --file-links only"
    );
}

#[test]
fn test_check_file_links_deterministic_order() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "beta.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta

[[file:beta-missing.org]]
"#,
    );
    db_write(
        &root,
        "alpha.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[file:zeta-missing.org]]
[[file:alpha-missing.org]]
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
    ]);

    assert!(!status.success());
    let observed: Vec<_> = v["broken_file_links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["source_uuid"].as_str().unwrap().to_string(),
                entry["source_title"].as_str().unwrap().to_string(),
                entry["target_path"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![
            (
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                "Alpha".to_string(),
                "alpha-missing.org".to_string(),
            ),
            (
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                "Alpha".to_string(),
                "zeta-missing.org".to_string(),
            ),
            (
                "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                "Beta".to_string(),
                "beta-missing.org".to_string(),
            ),
        ]
    );
}

#[test]
fn test_check_attachment_links_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "check",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(stdout.contains("Broken attach:"));
    assert!(stdout.contains("Attachment Link Note"));
}

#[test]
fn test_check_attachment_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(
        v.get("broken_attachment_links").is_some(),
        "expected broken_attachment_links field"
    );
    let broken_attach = v["broken_attachment_links"].as_array().unwrap();
    assert!(
        !broken_attach.is_empty(),
        "expected broken attachment links"
    );
    assert!(broken_attach[0]["source_title"].is_string());
    assert!(broken_attach[0]["target_path"].is_string());
    assert!(
        !broken_attach
            .iter()
            .any(|entry| entry["target_path"] == "image.png"),
        "org-attach hashed path should be treated as existing"
    );
    assert!(
        v.get("stats").is_none(),
        "stats should not appear with --attachment-links only"
    );
}

#[test]
fn test_check_attachment_links_deterministic_order() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "beta.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta

[[attachment:beta.png]]
"#,
    );
    db_write(
        &root,
        "alpha.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[attachment:zeta.png]]
[[attachment:alpha.png]]
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--attachment-links",
    ]);

    assert!(!status.success());
    let observed: Vec<_> = v["broken_attachment_links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["source_uuid"].as_str().unwrap().to_string(),
                entry["source_title"].as_str().unwrap().to_string(),
                entry["target_path"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![
            (
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                "Alpha".to_string(),
                "alpha.png".to_string(),
            ),
            (
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                "Alpha".to_string(),
                "zeta.png".to_string(),
            ),
            (
                "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                "Beta".to_string(),
                "beta.png".to_string(),
            ),
        ]
    );
}

#[test]
fn test_check_file_and_attachment_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(v.get("broken_file_links").is_some());
    assert!(v.get("broken_attachment_links").is_some());
    assert!(!v["broken_file_links"].as_array().unwrap().is_empty());
    assert!(!v["broken_attachment_links"].as_array().unwrap().is_empty());
    assert!(
        v.get("stats").is_none(),
        "stats should not appear without --stats flag"
    );
}

#[test]
fn test_check_id_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    assert!(!status.success());
    assert!(v["broken_links"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(v["broken_links"][0]["source_uuid"].is_string());
    assert!(
        v.get("broken_file_links").is_none(),
        "expected no file links with --id-links only"
    );
    assert!(
        v.get("broken_attachment_links").is_none(),
        "expected no attachment links with --id-links only"
    );
}

#[test]
fn test_check_rejects_agenda_flag() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "check", "--agenda"]);
    assert!(!status.success());
    assert!(stdout.is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("unexpected argument '--agenda'")
            || stderr.contains("unrecognized option '--agenda'"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_check_heading_equals_primary() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"),
        "scenario #2: heading equals own primary, got: {:?}",
        dups
    );
}

#[test]
fn test_check_heading_heading_same_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading One
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Heading Two
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #3: two headings share UUID, got: {:?}",
        dups
    );
}

#[test]
fn test_check_primary_equals_heading_other_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #4: primary of B equals heading of A, got: {:?}",
        dups
    );
}

#[test]
fn test_check_heading_heading_cross_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note B

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #5: heading in A equals heading in B, got: {:?}",
        dups
    );
}

#[test]
fn test_check_self_link_uuid() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "self_link.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Self-Link Note

[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa]]
"#,
    );
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--self-links",
    ]);
    assert!(!status.success(), "check should report issues");
    let self_links = v["self_links"].as_array().unwrap();
    assert_eq!(self_links.len(), 1);
    assert_eq!(self_links[0]["source_title"], "Self-Link Note");
    assert_eq!(self_links[0]["link_type"], "id");
    assert_eq!(
        self_links[0]["target"],
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
    );
    assert!(self_links[0]["suggestion"].is_null());
}

#[test]
fn test_check_self_link_file() {
    let (_dir, root) = setup_clean_db();
    std::fs::write(
        root.join("self_file.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Self-File Note

[[file:self_file.org]]
"#,
    )
    .unwrap();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--self-links",
    ]);
    assert!(!status.success(), "check should report issues");
    let self_links = v["self_links"].as_array().unwrap();
    assert_eq!(self_links.len(), 1);
    assert_eq!(self_links[0]["source_title"], "Self-File Note");
    assert_eq!(self_links[0]["link_type"], "file");
    assert_eq!(self_links[0]["target"], "self_file.org");
}

#[test]
fn test_check_self_link_heading_self_link_detected() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb]]
"#,
    );
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--self-links",
    ]);
    assert!(
        !status.success(),
        "heading linking to its own UUID should be detected as self-link"
    );
    let self_links = v["self_links"].as_array().unwrap();
    assert_eq!(
        self_links.len(),
        1,
        "expected heading self-link, got: {:?}",
        self_links
    );
    assert_eq!(self_links[0]["link_type"], "id");
    assert_eq!(
        self_links[0]["target"],
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"
    );
}

#[test]
fn test_check_self_link_file_with_headings_suggestion() {
    let (_dir, root) = setup_clean_db();
    std::fs::write(
        root.join("note.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note with Heading

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:

[[file:note.org]]
"#,
    )
    .unwrap();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--self-links",
    ]);
    assert!(!status.success(), "check should report issues");
    let self_links = v["self_links"].as_array().unwrap();
    assert_eq!(self_links.len(), 1);
    assert_eq!(self_links[0]["link_type"], "file");
    assert!(
        self_links[0]["suggestion"].is_string(),
        "expected suggestion when file has heading UUIDs"
    );
    let suggestion = self_links[0]["suggestion"].as_str().unwrap();
    assert!(
        suggestion.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"),
        "suggestion should mention primary UUID: {}",
        suggestion
    );
}

#[test]
fn test_check_self_link_no_false_positive() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb]]
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B
"#,
    );
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--self-links",
    ]);
    assert!(status.success(), "cross-links should not be self-links");
    let self_links = v["self_links"].as_array().unwrap();
    assert!(
        self_links.is_empty(),
        "expected no self-links, got: {:?}",
        self_links
    );
}

#[test]
fn test_check_self_link_implicit_with_all_checks() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "self_link.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Self-Link Note

[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa]]
"#,
    );
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
    ]);
    assert!(!status.success());
    let self_links = v["self_links"].as_array().unwrap();
    assert_eq!(
        self_links.len(),
        1,
        "self-link should be reported even without --self-links flag"
    );
}

#[test]
fn test_heading_uuid_duplicate_detection() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");

    fs::write(
        note_dir.join("dup-heading-1.org"),
        r#":PROPERTIES:
:ID:       dededede-dede-4ded-8ded-dededededede
:END:
#+title: Dup Heading 1

* Section
:PROPERTIES:
:ID:       efefefef-efef-4efe-8efe-efefefefefef
:END:
"#,
    )
    .unwrap();

    fs::write(
        note_dir.join("dup-heading-2.org"),
        r#":PROPERTIES:
:ID:       fafafafa-fafa-4faf-8faf-fafafafafafa
:END:
#+title: Dup Heading 2

* Section
:PROPERTIES:
:ID:       efefefef-efef-4efe-8efe-efefefefefef
:END:
"#,
    )
    .unwrap();

    let (v, status) = run_json(&["--db", db, "--output-format", "json", "check", "--id-links"]);
    assert!(!status.success(), "check should report issues");
    let dups = v["duplicates"]["duplicate_uuids"].as_array().unwrap();
    let clash = dups
        .iter()
        .find(|d| d["value"] == "efefefef-efef-4efe-8efe-efefefefefef");
    assert!(
        clash.is_some(),
        "should report clashing heading UUID, got dups: {:?}",
        dups
    );
}

#[test]
fn test_check_self_link_human_output() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "self_link.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Self-Link Note

[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa]]
"#,
    );
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check"]);
    assert!(!status.success());
    assert!(
        stdout.contains("Self-referencing links"),
        "human output should mention self-referencing links, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Self-Link Note"),
        "human output should mention the note title, got: {}",
        stdout
    );
}
