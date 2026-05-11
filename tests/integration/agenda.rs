use super::*;

#[test]
fn test_agenda_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda"]);
    assert!(status.success(), "agenda failed: {stdout}");
    assert!(stdout.contains("planned item"), "stdout: {stdout}");
}

#[test]
fn test_agenda_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    assert!(
        v["items"].as_array().unwrap().len() >= 1,
        "expected at least 1 agenda item"
    );
}

#[test]
fn test_agenda_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "agenda",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
        assert!(v.get("todo_state").is_some());
    }
}

#[test]
fn test_agenda_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--include",
        "TODO",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let todo_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("TODO"))
        .collect();
    assert!(
        todo_items.len() >= 1,
        "expected TODO items with --include TODO"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "TODO", "expected all items to be TODO");
    }
}

#[test]
fn test_agenda_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
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
fn test_agenda_today_and_week() {
    let (_dir, root) = setup_db();
    let (_, _stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda", "--today"]);
    assert!(status.success());

    let (_, _stderr2, status2) = run(&["--db", root.to_str().unwrap(), "agenda", "--week"]);
    assert!(status2.success());
}
