use super::*;

#[test]
fn test_show_basic() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "show",
        "--uuid",
        "h5h5h5h5-h5h5-4h5h-h5h5-h5h5h5h5h5h5",
    ]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("Parent task"));
    assert!(stdout.contains("TODO"));
    assert!(stdout.contains("Scheduled:"));
    assert!(stdout.contains("Subtasks (blocks):"));
    assert!(stdout.contains("Child task A"));
    assert!(stdout.contains("Child task B"));
    assert!(stdout.contains("Grandchild"));
}

#[test]
fn test_show_canonical_id() {
    let (_dir, root) = setup_db();
    // ID 1 from pkms todo should resolve to some task
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "show", "1"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("Task:"));
    assert!(stdout.contains("File:"));
    assert!(stdout.contains("State:"));
}

#[test]
fn test_show_first_todo() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "show", "Agenda Item"]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("High priority task"));
    assert!(stdout.contains("[#A]"));
    assert!(stdout.contains("agenda"));
}

#[test]
fn test_show_json() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "show",
        "--uuid",
        "h5h5h5h5-h5h5-4h5h-h5h5-h5h5h5h5h5h5",
    ]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["heading_title"], "Parent task");
    assert_eq!(v["todo_state"], "TODO");
    assert!(v["parents"].as_array().unwrap().is_empty());
    assert!(v["children"].as_array().unwrap().len() >= 2);
    assert!(v["content"].as_str().unwrap().contains("Parent task"));
}

#[test]
fn test_show_canonical_id_json() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "show",
        "1",
    ]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v["heading_title"].as_str().is_some());
    assert!(v["line_number"].as_u64().is_some());
    assert!(v["path"].as_str().is_some());
}

#[test]
fn test_show_no_todo_heading() {
    let (_dir, root) = setup_db();
    let (_stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "show", "Note A"]);
    assert!(!status.success());
    assert!(stderr.contains("No TODO heading"));
}

#[test]
fn test_show_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, stderr, status) =
        run(&["--db", root.to_str().unwrap(), "show", "NonExistentNote"]);
    assert!(!status.success());
    assert!(stderr.contains("not found") || stderr.contains("NonExistentNote"));
}

#[test]
fn test_show_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "show",
        "--uuid",
        "h5h5h5h5-h5h5-4h5h-h5h5-h5h5h5h5h5h5",
    ]);
    assert!(status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let line = stdout.trim();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["heading_title"], "Parent task");
}

#[test]
fn test_show_invalid_canonical_id() {
    let (_dir, root) = setup_db();
    let (_stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "show", "99999"]);
    assert!(!status.success());
    assert!(stderr.contains("No task with canonical ID"));
}
