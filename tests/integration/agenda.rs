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
fn test_agenda_state_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--state",
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
        "expected TODO items with --state TODO"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "TODO", "expected all items to be TODO");
    }
}

#[test]
fn test_agenda_state_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--state",
        "!DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.is_empty(),
        "expected no DONE items with --state !DONE"
    );
}

#[test]
fn test_agenda_columns_subset() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "agenda",
        "--columns",
        "Date,Type,Heading",
    ]);
    assert!(status.success(), "agenda --columns failed: {stdout}");
    assert!(stdout.contains("Date"), "expected Date column");
    assert!(stdout.contains("Type"), "expected Type column");
    assert!(stdout.contains("Heading"), "expected Heading column");
    assert!(!stdout.contains("State"), "should not have State column");
    assert!(!stdout.contains("Tags"), "should not have Tags column");
}

#[test]
fn test_agenda_columns_json_unaffected() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--columns",
        "Date,Note",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    let item = &v["items"].as_array().unwrap()[0];
    assert!(
        item.get("todo_state").is_some(),
        "JSON should have todo_state"
    );
}

#[test]
fn test_agenda_tags_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--tags",
        "agenda",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        let filetags = item["filetags"].as_array().unwrap();
        assert!(
            filetags.iter().any(|t| t.as_str() == Some("agenda")),
            "expected all items to have agenda tag"
        );
    }
}

#[test]
fn test_agenda_tags_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--tags",
        "!agenda",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        let filetags = item["filetags"].as_array().unwrap();
        assert!(
            !filetags.iter().any(|t| t.as_str() == Some("agenda")),
            "expected no items with agenda tag"
        );
    }
}

#[test]
fn test_agenda_type_sched() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--type",
        "SCHED",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        assert!(
            item["scheduled"].is_string(),
            "expected all items with SCHEDULED"
        );
    }
}

#[test]
fn test_agenda_type_deadl() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--type",
        "DEADL",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        assert!(
            item["deadline"].is_string(),
            "expected all items with DEADLINE"
        );
    }
}

#[test]
fn test_agenda_prio_a() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--prio",
        "A",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        assert_eq!(
            item["priority"].as_str(),
            Some("A"),
            "expected all items with priority A"
        );
    }
}

#[test]
fn test_agenda_sort_multi() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--sort",
        "date,priority",
    ]);
    assert!(status.success());
    assert!(v.get("items").is_some(), "expected items field");
}

#[test]
fn test_agenda_today_and_week() {
    let (_dir, root) = setup_db();
    let (_, _stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda", "--today"]);
    assert!(status.success());

    let (_, _stderr2, status2) = run(&["--db", root.to_str().unwrap(), "agenda", "--week"]);
    assert!(status2.success());
}

#[test]
fn test_agenda_open_valid_id() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda", "--open", "1"]);
    assert!(status.success(), "agenda --open 1 failed: stderr={stderr}");
    assert!(stdout.contains("Opening #1"), "stdout: {stdout}");
}

#[test]
fn test_agenda_open_invalid_id() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) =
        run(&["--db", root.to_str().unwrap(), "agenda", "--open", "9999"]);
    assert!(!status.success(), "expected failure for invalid ID");
    assert!(stderr.contains("No task with ID 9999"), "stderr: {stderr}");
    let _ = stdout;
}
