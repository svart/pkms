use super::*;

#[test]
fn test_validate_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "validate", "Note A"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("healthy"));
}

#[test]
fn test_validate_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note A",
    ]);
    assert!(status.success());
    assert_eq!(v["title"], "Note A");
    assert_eq!(v["healthy"], true);
    assert!(v.get("uuid").is_some());
    assert!(v.get("outgoing").is_some());
    assert!(v.get("incoming").is_some());
}

#[test]
fn test_validate_accepts_uppercase_title_keyword() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "uppercase-title.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+TITLE: Uppercase Title
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Uppercase Title",
    ]);
    assert!(status.success(), "validate failed: {v}");
    assert_eq!(v["healthy"], true, "uppercase #+TITLE should be valid: {v}");
}

#[test]
fn test_validate_broken_note() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Broken Note",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], false);
    assert!(v["broken_internal"].as_array().map_or(0, |a| a.len()) >= 1);
}

#[test]
fn test_validate_broken_file_link_json() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "source.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Source

[[file:target.org::needle]]
[[file:target.org::missing]]
[[attachment:missing.png]]
"#,
    );
    db_write(
        &root,
        "target.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Target

needle
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Source",
    ]);

    assert!(status.success());
    assert_eq!(v["healthy"], false);
    assert_eq!(
        v["broken_files"].as_array().unwrap(),
        &[serde_json::json!("target.org::missing")]
    );
    let issues: Vec<_> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|issue| issue.as_str())
        .collect();
    assert!(issues.contains(&"1 broken file link(s)"));
    assert!(
        issues.iter().all(|issue| !issue.contains("attachment")),
        "validate should not report attachment links: {:?}",
        issues
    );
}

#[test]
fn test_validate_skips_ssh_file_targets() {
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
        "validate",
        "Remote",
    ]);

    assert!(status.success(), "validate failed: {v}");
    assert_eq!(v["healthy"], true);
    assert_eq!(v["broken_files"].as_array().unwrap().len(), 0);
}

#[test]
fn test_validate_note_not_found() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "validate",
        "NonexistentNote",
    ]);
    assert!(!status.success());
    assert!(
        stderr.contains("not found") || stdout.contains("error"),
        "stderr: {}\nstdout: {}",
        stderr,
        stdout
    );
}

#[test]
fn test_validate_bad_filetags() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Bad Filetags Note",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], false);
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .filter(|i| i.contains("filetags"))
        .collect();
    assert!(
        issues.len() >= 2,
        "expected at least 2 filetags issues, got {:?}",
        issues
    );
}

#[test]
fn test_validate_ignores_missing_agenda_filetag() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Missing Agenda Tag",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    let has_agenda_issue = issues
        .iter()
        .any(|i| i.as_str().is_some_and(|s| s.contains("agenda")));
    assert!(
        !has_agenda_issue,
        "validate should not report missing :agenda: filetag, issues: {:?}",
        issues
    );
    assert_eq!(v["healthy"], true);
}

#[test]
fn test_validate_heading_uuids_field() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("validate-heading-uuids.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       bebebebe-bebe-4beb-8beb-bebebebebebe
:END:
#+title: Validate Heading UUIDs

* Section A
:PROPERTIES:
:ID:       cacacaca-caca-4cac-8cac-cacacacacaca
:END:
Text
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "validate",
        "Validate Heading UUIDs",
    ]);
    assert!(status.success());
    assert!(v.get("heading_uuids").is_some());
    let heading_uuids = v["heading_uuids"].as_array().unwrap();
    assert_eq!(heading_uuids.len(), 1);
    assert_eq!(heading_uuids[0], "cacacaca-caca-4cac-8cac-cacacacacaca");
}

#[test]
fn test_link_to_heading_resolves() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");

    fs::write(
        note_dir.join("heading-target.org"),
        r#":PROPERTIES:
:ID:       abababab-abab-4aba-8aba-abababababab
:END:
#+title: Heading Target

* Target Section
:PROPERTIES:
:ID:       bcbcbcbc-bcbc-4bbc-8bbc-bcbcbcbcbcbc
:END:
"#,
    )
    .unwrap();

    fs::write(
        note_dir.join("heading-linker.org"),
        r#":PROPERTIES:
:ID:       cdcdcdcd-cdcd-4cdc-8cdc-cdcdcdcdcdcd
:END:
#+title: Heading Linker

[[id:bcbcbcbc-bcbc-4bbc-8bbc-bcbcbcbcbcbc][Link to heading]]
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "validate",
        "Heading Linker",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], true);
    assert!(
        v["broken_internal"].as_array().unwrap().is_empty(),
        "link to heading UUID should not be broken"
    );
}

// --- heading UUID systematic coverage ---

#[test]
fn test_validate_heading_equals_primary() {
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
        "validate",
        "Note",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #2: validate should detect heading equals primary"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("Duplicate UUID")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_heading_heading_same_file() {
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
        "validate",
        "Note",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #3: validate should detect two headings sharing UUID"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues
            .iter()
            .any(|i| i.contains("used by multiple headings")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_primary_equals_heading_other_file() {
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
        "validate",
        "Note A",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #4: validate should detect heading matching another note's primary"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("belongs to another note")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_heading_heading_cross_file() {
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
        "validate",
        "Note B",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #5: validate should detect heading matching another note's heading"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("belongs to another note")),
        "validate issues: {:?}",
        issues
    );
}

// --- self-link validate tests ---

#[test]
fn test_validate_self_link_uuid() {
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
        "validate",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.as_str().unwrap().contains("Self-link")),
        "expected self-link issue, got: {:?}",
        issues
    );
    assert_eq!(v["healthy"], false);
}

#[test]
fn test_validate_self_link_file() {
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
        "validate",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.as_str().unwrap().contains("Self-link")),
        "expected self-link issue, got: {:?}",
        issues
    );
    assert_eq!(v["healthy"], false);
}

#[test]
fn test_validate_self_link_heading_uuid_detected() {
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
        "validate",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.as_str().unwrap().contains("Self-link")),
        "heading UUID linking to itself should be detected, got: {:?}",
        issues
    );
    assert_eq!(v["healthy"], false);
}

#[test]
fn test_validate_self_link_file_with_heading_suggestion() {
    let (_dir, root) = setup_clean_db();
    std::fs::write(
        root.join("note.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

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
        "validate",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    let suggestion_issue = issues.iter().find(|i| {
        i.as_str()
            .unwrap()
            .contains("consider using id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
    });
    assert!(
        suggestion_issue.is_some(),
        "expected suggestion to use UUID, got: {:?}",
        issues
    );
}
