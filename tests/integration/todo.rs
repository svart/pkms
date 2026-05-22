use super::*;

#[test]
fn test_todo_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "todo"]);
    assert!(status.success(), "todo failed: {stdout}");
    assert!(stdout.contains("TODO"), "stdout: {stdout}");
}

#[test]
fn test_todo_dateless_items_visible_in_text() {
    let (_dir, root) = setup_db();
    let (v, _) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
    ]);
    let json_total = v["items"].as_array().unwrap().len();

    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "todo"]);
    assert!(status.success(), "todo failed: {stdout}");

    // Items known to have no SCHEDULED/DEADLINE/daily_file_date should appear
    // in text output
    assert!(
        stdout.contains("Completed task"),
        "dateless DONE item missing"
    );
    assert!(stdout.contains("Review"), "dateless WAITING item missing");
    assert!(stdout.contains("Something"), "dateless IDEA item missing");
    assert!(
        stdout.contains("Child task B"),
        "dateless TODO child missing"
    );
    assert!(stdout.contains("Grandchild"), "dateless DONE child missing");
    assert!(
        stdout.contains("Another top task"),
        "dateless TODO item missing"
    );

    // The footer should say "Total:" (not "Shown:") and the count should
    // match the JSON item count
    let footer_line = stdout.lines().last().unwrap_or("");
    assert!(
        footer_line.starts_with("Total:"),
        "expected footer to show Total, got: {footer_line}"
    );
    assert!(
        footer_line.contains(&json_total.to_string()),
        "expected footer count {} in: {footer_line}",
        json_total
    );
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
        !v["items"].as_array().unwrap().is_empty(),
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
fn test_todo_state_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--state",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        !done_items.is_empty(),
        "expected DONE items with --state DONE"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "DONE", "expected all items to be DONE");
    }
}

#[test]
fn test_todo_state_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
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
fn test_todo_tags_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--tags",
        "daily",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        let filetags = item["filetags"].as_array().unwrap();
        assert!(
            filetags.iter().any(|t| t.as_str() == Some("daily")),
            "expected all items to have daily tag"
        );
    }
}

#[test]
fn test_todo_tags_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--tags",
        "!daily",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    for item in items {
        let filetags = item["filetags"].as_array().unwrap();
        assert!(
            !filetags.iter().any(|t| t.as_str() == Some("daily")),
            "expected no items with daily tag"
        );
    }
}

#[test]
fn test_todo_type_sched() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
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
fn test_todo_type_deadl() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
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
fn test_todo_sort_multi() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--sort",
        "state,date",
    ]);
    assert!(status.success());
    assert!(v.get("items").is_some(), "expected items field");
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
fn test_todo_and_agenda_share_canonical_ids_for_same_headings() {
    let (_dir, root) = setup_db();
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
    ]);
    assert!(todo_status.success());
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
    ]);
    assert!(agenda_status.success());

    let mut todo_ids_by_heading = std::collections::HashMap::new();
    for item in todo["items"].as_array().unwrap() {
        let key = (
            item["path"].as_str().unwrap().to_string(),
            item["line_number"].as_u64().unwrap(),
        );
        todo_ids_by_heading.insert(key, item["id"].as_u64().unwrap());
    }

    let agenda_items = agenda["items"].as_array().unwrap();
    assert!(
        !agenda_items.is_empty(),
        "fixture should contain agenda-backed tasks"
    );
    for item in agenda_items {
        let key = (
            item["path"].as_str().unwrap().to_string(),
            item["line_number"].as_u64().unwrap(),
        );
        assert_eq!(
            todo_ids_by_heading.get(&key).copied(),
            Some(item["id"].as_u64().unwrap()),
            "agenda item should use the same canonical ID as todo: {}",
            item["heading_title"].as_str().unwrap()
        );
    }
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
    assert_eq!(groups["TODO"].as_array().unwrap().len(), 9);
    assert_eq!(groups["DONE"].as_array().unwrap().len(), 2);
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
    assert_eq!(groups["No Priority"].as_array().unwrap().len(), 13);
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
        3,
        "expected 3 items from Daily Note + Daily Plan scope"
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
fn test_todo_after() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--after",
        "2026-05-10",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    // Items with dates >= 2026-05-10: High priority task (SCHEDULED 2026-05-10),
    // Low priority task (DEADLINE 2026-06-15)
    assert!(
        items.len() >= 2,
        "expected at least 2 items with --after 2026-05-10, got {}",
        items.len()
    );
}

#[test]
fn test_todo_before() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--before",
        "2026-05-05",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    // Items with dates <= 2026-05-05: Morning routine (SCHEDULED 2026-05-03),
    // Project work (DEADLINE 2026-05-05)
    assert!(
        items.len() >= 2,
        "expected at least 2 items with --before 2026-05-05, got {}",
        items.len()
    );
}

#[test]
fn test_todo_after_before_range() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--after",
        "2026-06-01",
        "--before",
        "2026-06-30",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        3,
        "expected 3 items in date range, got {}",
        items.len()
    );
}

#[test]
fn test_todo_prio_a() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--prio",
        "A",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "expected 1 item with priority A, got {}",
        items.len()
    );
    assert_eq!(items[0]["heading_title"], "High priority task");
}

#[test]
fn test_todo_prio_empty() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--prio",
        "",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    // Every item without priority should be included
    for item in items {
        assert!(
            item["priority"].is_null(),
            "expected no priority for item {:?}, got {:?}",
            item["heading_title"],
            item["priority"]
        );
    }
}

#[test]
fn test_todo_after_datetime() {
    let (_dir, root) = setup_db();
    // Add a note with a SCHEDULED timestamp that includes time
    db_write(
        &root,
        "20260101000015-timed_note.org",
        r#":PROPERTIES:
:ID:       5555aaaa-5555-4555-8555-555555555555
:END:
#+title: Timed Note
#+filetags: :test:

* TODO Morning task
SCHEDULED: <2026-05-10 Sun 09:00>
* TODO Afternoon task
SCHEDULED: <2026-05-10 Sun 14:00>
"#,
    );
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--after",
        "2026-05-10 12:00",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    // Only "Afternoon task" (SCHEDULED 2026-05-10 14:00) should match
    // "Morning task" (SCHEDULED 2026-05-10 09:00) is before 12:00
    let heading_titles: Vec<&str> = items
        .iter()
        .filter_map(|i| i["heading_title"].as_str())
        .collect();
    assert!(
        heading_titles.contains(&"Afternoon task"),
        "expected Afternoon task in results, got {:?}",
        heading_titles
    );
    assert!(
        !heading_titles.contains(&"Morning task"),
        "did not expect Morning task (09:00 < 12:00), got {:?}",
        heading_titles
    );
}

#[test]
fn test_todo_columns_subset() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "todo",
        "--columns",
        "Date,Note",
    ]);
    assert!(status.success(), "todo --columns failed: {stdout}");
    assert!(stdout.contains("Date"), "expected Date column");
    assert!(stdout.contains("Note"), "expected Note column");
    assert!(!stdout.contains("State"), "should not have State column");
    assert!(!stdout.contains("Prio"), "should not have Prio column");
}

#[test]
fn test_todo_columns_single() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "todo",
        "--columns",
        "Heading",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Heading"));
    assert!(!stdout.contains("Date"));
    assert!(stdout.contains("Total:"));
}

#[test]
fn test_todo_columns_json_unaffected() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--columns",
        "Date,Note",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    assert!(
        !v["items"].as_array().unwrap().is_empty(),
        "expected at least 1 todo item"
    );
    // JSON should still contain all fields
    let item = &v["items"].as_array().unwrap()[0];
    assert!(
        item.get("todo_state").is_some(),
        "JSON should have todo_state"
    );
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
