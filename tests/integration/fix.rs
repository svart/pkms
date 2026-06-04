use super::*;

#[test]
fn test_fix_dry_run() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
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
    assert!(ignored.contains(broken), "ignored file should not be changed");
}
