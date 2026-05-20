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
