use super::*;

#[test]
fn test_todo_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "todo"]);
    assert!(status.success(), "todo failed: {stdout}");
    assert!(stdout.contains("TODO"), "stdout: {stdout}");
}

#[test]
fn test_todo_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    assert!(
        v["items"].as_array().unwrap().len() >= 1,
        "expected at least 1 todo item"
    );
}

#[test]
fn test_todo_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "todo",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
        assert!(v.get("todo_state").is_some());
    }
}

#[test]
fn test_todo_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--include",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.len() >= 1,
        "expected DONE items with --include DONE"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "DONE", "expected all items to be DONE");
    }
}

#[test]
fn test_todo_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--exclude",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.is_empty(),
        "expected no DONE items with --exclude DONE"
    );
}

#[test]
fn test_todo_sort_state() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "todo", "--sort", "state"]);
    assert!(status.success(), "todo --sort state failed: {stdout}");
    assert!(
        stdout.contains("TODO") || stdout.contains("DONE"),
        "stdout: {stdout}"
    );
}
