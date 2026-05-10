use super::*;

#[test]
fn test_context_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "context",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success(), "stdout: {}", stdout);
    assert!(stdout.contains("Note A"), "stdout: {}", stdout);
}

#[test]
fn test_context_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "context",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success());
    assert_eq!(v["target"], "Note A");
    assert!(v.get("context").is_some());
    assert!(v.get("estimated_tokens").is_some());
    assert!(v["context"].as_str().unwrap_or("").contains("Note B"));
}

#[test]
fn test_context_depth_2() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "context",
        "Note A",
        "--depth",
        "2",
    ]);
    assert!(status.success());
    assert!(v["context"].as_str().unwrap_or("").contains("Note C"));
}

#[test]
fn test_context_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "context", "Nonexistent"]);
    assert!(!status.success());
}
