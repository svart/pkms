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

#[test]
fn test_todo_group_state() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--group",
        "state",
    ]);
    assert!(status.success());
    assert_eq!(v["group_field"], "state");
    let groups = v["groups"].as_object().unwrap();
    assert!(groups.contains_key("TODO"), "expected TODO group");
    assert!(groups.contains_key("DONE"), "expected DONE group");
    assert_eq!(groups["TODO"].as_array().unwrap().len(), 5);
    assert_eq!(groups["DONE"].as_array().unwrap().len(), 1);
}

#[test]
fn test_todo_group_file() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--group",
        "file",
    ]);
    assert!(status.success());
    assert_eq!(v["group_field"], "file");
    let groups = v["groups"].as_object().unwrap();
    assert!(
        groups.contains_key("Agenda Item"),
        "expected Agenda Item group"
    );
    assert!(
        groups.contains_key("Daily Note"),
        "expected Daily Note group"
    );
    assert_eq!(groups["Agenda Item"].as_array().unwrap().len(), 3);
}

#[test]
fn test_todo_group_priority() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--group",
        "priority",
    ]);
    assert!(status.success());
    assert_eq!(v["group_field"], "priority");
    let groups = v["groups"].as_object().unwrap();
    assert!(
        groups.contains_key("Priority A"),
        "expected Priority A group"
    );
    assert!(
        groups.contains_key("No Priority"),
        "expected No Priority group"
    );
    assert_eq!(groups["Priority A"].as_array().unwrap().len(), 1);
    assert_eq!(groups["No Priority"].as_array().unwrap().len(), 5);
}

#[test]
fn test_todo_group_sort_within() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--group",
        "state",
        "--sort",
        "file",
    ]);
    assert!(status.success());
    assert_eq!(v["group_field"], "state");
    let groups = v["groups"].as_object().unwrap();
    assert!(groups.contains_key("TODO"));
    let todo_items = groups["TODO"].as_array().unwrap();
    // Should be sorted by file (title) within the TODO group
    assert_eq!(todo_items[0]["title"].as_str().unwrap(), "Agenda Item");
}

#[test]
fn test_todo_group_text_output() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "todo", "--group", "state"]);
    assert!(status.success(), "todo --group state text failed: {stdout}");
    assert!(stdout.contains("=== TODO"));
    assert!(stdout.contains("=== DONE"));
    assert!(stdout.contains("Total:"));
}

#[test]
fn test_todo_scope_uuid() {
    let (_dir, root) = setup_db();
    let uuid = "f3f3f3f3-f3f3-4f3f-f3f3-f3f3f3f3f3f3";
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        uuid,
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "expected 3 items from Agenda Item scope");
    for item in items {
        assert_eq!(item["uuid"].as_str().unwrap(), uuid);
    }
}

#[test]
fn test_todo_scope_title() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        "Agenda Item",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "expected 3 items from Agenda Item scope");
}

#[test]
fn test_todo_scope_multiple() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        "Daily Note",
        "Daily Plan",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "expected 2 items from Daily Note + Daily Plan scope"
    );
}

#[test]
fn test_todo_scope_empty_result() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        "Note A",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(items.is_empty(), "expected no todo items for Note A");
}

#[test]
fn test_todo_scope_group() {
    let (_dir, root) = setup_db();
    let uuid = "f3f3f3f3-f3f3-4f3f-f3f3-f3f3f3f3f3f3";
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        uuid,
        "--group",
        "state",
    ]);
    assert!(status.success());
    assert_eq!(v["group_field"], "state");
    let groups = v["groups"].as_object().unwrap();
    assert!(groups.contains_key("TODO"));
    assert!(groups.contains_key("DONE"));
    assert_eq!(groups["TODO"].as_array().unwrap().len(), 2);
    assert_eq!(groups["DONE"].as_array().unwrap().len(), 1);
}

#[test]
fn test_todo_scope_ndjson() {
    let (_dir, root) = setup_db();
    let uuid = "f3f3f3f3-f3f3-4f3f-f3f3-f3f3f3f3f3f3";
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "todo",
        "--scope",
        uuid,
    ]);
    assert!(status.success());
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3);
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(v["uuid"].as_str().unwrap(), uuid);
    }
}
