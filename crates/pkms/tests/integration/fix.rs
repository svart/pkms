use super::*;

#[test]
fn test_fix_dry_run() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "uuid",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Would fix") || stdout.contains("use --apply"));
}

#[test]
fn test_fix_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "uuid",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert_eq!(v["broken_uuid"], "ffffffff-ffff-4fff-ffff-ffffffffffff");
    assert_eq!(v["applied"], false);
}

#[test]
fn test_fix_invalid_uuid_json_error() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "uuid",
        "not-a-uuid",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v.get("error").is_some());
}

#[test]
fn test_fix_broken_not_found() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "uuid",
        "00000000-0000-0000-0000-000000000000",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert!(stdout.contains("0 broken link") || stdout.contains("Would fix"));
}

#[test]
fn test_fix_apply_human_output() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "uuid",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--apply",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("Fixed"),
        "Applied fix human output should say 'Fixed', got: {stdout}"
    );
    assert!(
        stdout.contains("broken link"),
        "Output should mention link count, got: {stdout}"
    );
}

#[test]
fn test_fix_apply_json_output() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "uuid",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--apply",
    ]);
    assert!(status.success());
    assert_eq!(v["applied"], true, "fix should be applied, got: {v}");
}

#[test]
fn test_fix_apply_honors_ignore_patterns() {
    let (_dir, root) = setup_clean_db();
    let broken = "ffffffff-ffff-4fff-ffff-ffffffffffff";
    let replacement = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";

    db_write(
        &root,
        "target.org",
        &format!(
            r#":PROPERTIES:
:ID:       {replacement}
:END:
#+title: Target
"#
        ),
    );
    db_write(
        &root,
        "source.org",
        &format!(
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Source

[[id:{broken}][Broken]]
"#
        ),
    );
    db_write(
        &root,
        "ignored/backup.org",
        &format!(
            r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Ignored Backup

[[id:{broken}][Broken]]
"#
        ),
    );

    let (stdout, _stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "fix",
            "uuid",
            broken,
            replacement,
            "--apply",
        ],
        r#"ignore_patterns = ["ignored"]
"#,
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(status.success(), "fix failed: {v}");
    assert_eq!(v["total_replacements"], 1);

    let source = fs::read_to_string(root.join("roam/source.org")).unwrap();
    assert!(source.contains(replacement));
    let ignored = fs::read_to_string(root.join("roam/ignored/backup.org")).unwrap();
    assert!(
        ignored.contains(broken),
        "ignored file should not be changed"
    );
}

#[test]
fn test_fix_attach_dry_run_json_reports_repair_without_moving() {
    let (_dir, root) = setup_clean_db();
    let note_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    let heading_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
    let target = "data/report.pdf";

    db_write(
        &root,
        "source.org",
        &format!(
            r#":PROPERTIES:
:ID:       {note_uuid}
:END:
#+title: Source

* Heading
:PROPERTIES:
:ID:       {heading_uuid}
:END:
[[attachment:{target}][Report]]
"#
        ),
    );
    let misplaced = root.join(".attach").join(note_uuid).join(target);
    fs::create_dir_all(misplaced.parent().unwrap()).unwrap();
    fs::write(&misplaced, b"report").unwrap();

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "attach",
    ]);

    assert!(status.success(), "fix attach failed: {v}");
    assert_eq!(v["applied"], false);
    assert_eq!(v["mode"], "move");
    assert_eq!(v["total_repaired"], 1);
    assert_eq!(v["total_skipped"], 0);
    assert_eq!(v["repairs"][0]["source_uuid"], heading_uuid);
    assert_eq!(v["repairs"][0]["target_path"], target);
    assert!(misplaced.exists(), "dry-run should leave source in place");
}

#[test]
fn test_fix_attach_apply_moves_file_to_heading_attach_dir() {
    let (_dir, root) = setup_clean_db();
    let note_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    let heading_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
    let target = "data/report.pdf";

    db_write(
        &root,
        "source.org",
        &format!(
            r#":PROPERTIES:
:ID:       {note_uuid}
:END:
#+title: Source

* Heading
:PROPERTIES:
:ID:       {heading_uuid}
:END:
[[attachment:{target}][Report]]
"#
        ),
    );
    let misplaced = root.join(".attach").join(note_uuid).join(target);
    fs::create_dir_all(misplaced.parent().unwrap()).unwrap();
    fs::write(&misplaced, b"report").unwrap();
    let expected = root
        .join(".attach")
        .join(&heading_uuid[..2])
        .join(&heading_uuid[2..])
        .join(target);

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "attach",
        "--apply",
    ]);

    assert!(status.success(), "fix attach failed: {v}");
    assert_eq!(v["applied"], true);
    assert_eq!(v["total_repaired"], 1);
    assert!(expected.exists(), "expected repaired file at {expected:?}");
    assert!(
        !misplaced.exists(),
        "move should remove the misplaced source"
    );
    let source = fs::read_to_string(root.join("roam/source.org")).unwrap();
    assert!(source.contains("[[attachment:data/report.pdf][Report]]"));
}

#[test]
fn test_fix_attach_skips_ambiguous_matches() {
    let (_dir, root) = setup_clean_db();
    let note_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    let heading_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
    let other_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
    let target = "report.pdf";

    db_write(
        &root,
        "source.org",
        &format!(
            r#":PROPERTIES:
:ID:       {note_uuid}
:END:
#+title: Source

* Heading
:PROPERTIES:
:ID:       {heading_uuid}
:END:
[[attachment:{target}][Report]]
"#
        ),
    );
    let first = root.join(".attach").join(note_uuid).join(target);
    let second = root.join(".attach").join(other_uuid).join(target);
    fs::create_dir_all(first.parent().unwrap()).unwrap();
    fs::create_dir_all(second.parent().unwrap()).unwrap();
    fs::write(&first, b"one").unwrap();
    fs::write(&second, b"two").unwrap();

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "attach",
        "--apply",
    ]);

    assert!(status.success(), "fix attach failed: {v}");
    assert_eq!(v["total_repaired"], 0);
    assert_eq!(v["total_skipped"], 1);
    assert_eq!(v["skipped"][0]["reason"], "ambiguous");
    assert!(first.exists());
    assert!(second.exists());
}
