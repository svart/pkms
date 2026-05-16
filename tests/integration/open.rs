use super::*;

#[test]
fn test_open_valid_id() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "open",
        "--editor",
        "true",
        "1",
    ]);
    assert!(status.success(), "open 1 failed: stderr={stderr}");
    assert!(stdout.contains("Opening:"), "stdout: {stdout}");
}

#[test]
fn test_open_invalid_id() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "open", "9999"]);
    assert!(!status.success(), "expected failure for invalid ID");
    assert!(
        stderr.contains("No task with canonical ID 9999"),
        "stderr: {stderr}"
    );
    let _ = stdout;
}

#[test]
fn test_open_by_uuid() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "open",
        "--editor",
        "true",
        "h5h5h5h5-h5h5-4h5h-h5h5-h5h5h5h5h5h5",
    ]);
    assert!(status.success(), "open by UUID failed: stderr={stderr}");
    assert!(stdout.contains("Opening:"), "stdout: {stdout}");
}

#[test]
fn test_open_invalid_target() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) =
        run(&["--db", root.to_str().unwrap(), "open", "nonexistent-target"]);
    assert!(!status.success(), "expected failure for invalid target");
    assert!(stderr.contains("Note not found"), "stderr: {stderr}");
    let _ = stdout;
}
