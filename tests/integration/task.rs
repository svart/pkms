use super::*;
#[cfg(feature = "todoist")]
use std::io::{Read, Write};
#[cfg(feature = "todoist")]
use std::net::TcpListener;
#[cfg(feature = "todoist")]
use std::process::Command;
#[cfg(feature = "todoist")]
use std::thread;

fn org_date(days_from_today: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(days_from_today))
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn test_task_help_lists_subcommands() {
    let (stdout, stderr, status) = run(&["task", "--help"]);
    assert!(status.success(), "task --help failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("list"));
    assert!(stdout.contains("agenda"));
    assert!(stdout.contains("today"));
    assert!(stdout.contains("overdue"));
    assert!(stdout.contains("upcoming"));
    assert!(stdout.contains("inbox"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("open"));
    assert!(stdout.contains("state"));
    assert!(stdout.contains("done"));
    assert!(stdout.contains("add"));
}

#[test]
fn test_task_list_matches_todo_count_json() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
    ]);
    assert!(task_status.success());
    assert!(todo_status.success());
    assert_eq!(
        task["items"].as_array().unwrap().len(),
        todo["items"].as_array().unwrap().len()
    );
    assert_eq!(task["items"][0]["source"], "pkms");
}

#[test]
fn test_task_list_limit_json_preserves_total_match_count() {
    let (_dir, root) = setup_db();
    let (all_tasks, all_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    let (limited, limited_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--limit",
        "1",
    ]);

    assert!(all_status.success());
    assert!(limited_status.success());
    assert_eq!(limited["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        limited["total"],
        all_tasks["items"].as_array().unwrap().len()
    );
}

#[test]
fn test_task_agenda_matches_agenda_count_json() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
    ]);
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
    ]);
    assert!(task_status.success());
    assert!(agenda_status.success());
    assert_eq!(
        task["items"].as_array().unwrap().len(),
        agenda["items"].as_array().unwrap().len()
    );
}

#[test]
fn test_task_today_matches_agenda_today_json() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    std::fs::write(
        root.join("roam/common/20260523000000-shortcut-today.org"),
        format!(
            r#":PROPERTIES:
:ID:       12121212-1212-4121-8121-121212121212
:END:
#+title: Shortcut Today
#+filetags: :agenda:

* TODO Shortcut today task
SCHEDULED: <{today}>
"#
        ),
    )
    .unwrap();
    let (shortcut, shortcut_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "today",
    ]);
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--today",
    ]);
    assert!(shortcut_status.success());
    assert!(agenda_status.success());
    assert_eq!(shortcut["items"], agenda["items"]);
}

#[test]
fn test_task_today_ndjson() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    std::fs::write(
        root.join("roam/common/20260523000003-shortcut-today-ndjson.org"),
        format!(
            r#":PROPERTIES:
:ID:       45454545-4545-4454-8454-454545454545
:END:
#+title: Shortcut Today Ndjson
#+filetags: :agenda:

* TODO Shortcut today ndjson task
SCHEDULED: <{today}>
"#
        ),
    )
    .unwrap();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "task",
        "today",
    ]);
    assert!(status.success());
    let first = stdout.lines().next().expect("expected at least one task");
    let v: serde_json::Value = serde_json::from_str(first).unwrap();
    assert_eq!(v["source"], "pkms");
}

#[test]
fn test_task_upcoming_days_filters_pkms_range() {
    let (_dir, root) = setup_db();
    let tomorrow = org_date(1);
    let later = org_date(8);
    std::fs::write(
        root.join("roam/common/20260523000001-shortcut-upcoming.org"),
        format!(
            r#":PROPERTIES:
:ID:       23232323-2323-4232-8232-232323232323
:END:
#+title: Shortcut Upcoming
#+filetags: :agenda:

* TODO Tomorrow task
SCHEDULED: <{tomorrow}>
* TODO Later task
SCHEDULED: <{later}>
"#
        ),
    )
    .unwrap();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "upcoming",
        "--days",
        "3",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert!(titles.contains(&"Tomorrow task"));
    assert!(!titles.contains(&"Later task"));
}

#[test]
fn test_task_overdue_accepts_limit_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "overdue",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert!(v["items"].as_array().unwrap().len() <= 1);
}

#[test]
fn test_task_list_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "task",
        "list",
    ]);
    assert!(status.success());
    let first = stdout.lines().next().expect("expected at least one task");
    let v: serde_json::Value = serde_json::from_str(first).unwrap();
    assert_eq!(v["source"], "pkms");
}

#[test]
fn test_task_list_rejects_todoist_filter_for_default_pkms_source() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "todoist.filter:today | overdue",
    ]);

    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("todoist.filter requires source:todoist or source:all")
    );
}

#[test]
fn test_task_list_rejects_todoist_filter_for_explicit_pkms_source() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "source:pkms",
        "todoist.filter:today | overdue",
    ]);

    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("todoist.filter requires source:todoist or source:all")
    );
}

#[cfg(not(feature = "todoist"))]
#[test]
fn test_task_todoist_source_rejected_before_todoist_support() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "source:todoist",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        v["error"],
        "Todoist support is not available in this build. Rebuild with --features todoist."
    );
}

#[test]
fn test_task_show_accepts_pkms_id_forms() {
    let (_dir, root) = setup_db();
    for id in ["1", "p1", "pkms:1"] {
        let (v, status) = run_json(&[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "show",
            id,
        ]);
        assert!(status.success(), "task show {id} failed");
        assert!(v.get("heading_title").is_some());
    }
}

#[test]
fn test_task_open_accepts_pkms_id_form() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "open",
        "p1",
        "--editor",
        "true",
    ]);
    assert!(status.success(), "task open failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Opening:"));
}

#[cfg(not(feature = "todoist"))]
#[test]
fn test_task_show_todoist_source_rejected_before_todoist_support() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "show",
        "todoist:123",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        v["error"],
        "Todoist support is not available in this build. Rebuild with --features todoist."
    );
}

#[test]
fn test_task_state_dry_run_does_not_edit_file() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "state",
        "p1",
        "waiting",
        "--dry-run",
    ]);
    assert!(status.success());
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "WAITING");
    assert_eq!(v["dry_run"], true);
    let path = v["path"].as_str().unwrap();
    let line_number = v["line_number"].as_u64().unwrap() as usize;
    let content = std::fs::read_to_string(path).unwrap();
    let line = content.lines().nth(line_number - 1).unwrap();
    assert!(line.contains("TODO"));
    assert!(!line.contains("WAITING"));
}

#[test]
fn test_task_state_writes_canonical_config_spelling() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "state",
        "p1",
        "waiting",
    ]);
    assert!(status.success());
    assert_eq!(v["new_state"], "WAITING");
    let path = v["path"].as_str().unwrap();
    let line_number = v["line_number"].as_u64().unwrap() as usize;
    let content = std::fs::read_to_string(path).unwrap();
    let line = content.lines().nth(line_number - 1).unwrap();
    assert!(line.contains("WAITING"));
    assert!(!line.contains("TODO"));
}

#[test]
fn test_task_done_writes_closed_state() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "done",
        "pkms:1",
    ]);
    assert!(status.success());
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "DONE");
}

#[test]
fn test_task_state_rejects_unknown_state() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "state",
        "p1",
        "UNKNOWN",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v["error"].as_str().unwrap().contains("Valid states:"));
}

#[cfg(not(feature = "todoist"))]
#[test]
fn test_task_state_rejects_todoist_before_todoist_support() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "state",
        "todoist:123",
        "DONE",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["error"], "Todoist task source is not implemented yet");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_uses_mock_api_and_pagination() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![
        (
            "GET",
            "/tasks?limit=200",
            r#"{"results":[{"id":"abc","content":"Buy milk","description":"","project_id":"inbox","priority":4,"labels":["errand"],"due":{"date":"2026-05-23","string":"today"},"url":"https://todoist.com/showTask?id=abc"}],"next_cursor":"next"}"#,
        ),
        (
            "GET",
            "/tasks?limit=200&cursor=next",
            r#"{"results":[{"id":"def","content":"Call Sam","description":"Discuss plan","priority":1,"labels":[]}],"next_cursor":null}"#,
        ),
        (
            "GET",
            "/projects?limit=200",
            r#"{"results":[{"id":"inbox","name":"Inbox"}],"next_cursor":null}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["source"], "todoist");
    assert_eq!(items[0]["source_id"], "abc");
    assert_eq!(items[0]["priority"], "A");
    assert_eq!(items[0]["project"], "Inbox");
    assert_eq!(items[0]["project_id"], "inbox");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_projects_todoist_lists_metadata() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/projects?limit=200",
        r#"{"results":[{"id":"inbox","name":"Inbox"},{"id":"work","name":"Work"}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "projects",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"][0]["id"], "inbox");
    assert_eq!(v["items"][0]["name"], "Inbox");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_labels_todoist_lists_metadata() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/labels?limit=200",
        r#"{"results":[{"id":"phone-id","name":"phone"},{"id":"errand-id","name":"errand"}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "labels",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"][1]["id"], "errand-id");
    assert_eq!(v["items"][1]["name"], "errand");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_filter_is_passed_to_mock_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today%20%7C%20overdue&limit=200",
        r#"{"results":[{"id":"abc","content":"Filtered task","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "source:todoist",
            "todoist.filter:today | overdue",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Filtered task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_today_uses_today_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today&limit=200",
        r#"{"results":[{"id":"today","content":"Today task","priority":1,"labels":[],"due":{"date":"2026-05-23","string":"today"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--today",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["source"], "todoist");
    assert_eq!(v["items"][0]["title"], "Today task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_overdue_uses_overdue_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=overdue&limit=200",
        r#"{"results":[{"id":"old","content":"Overdue task","priority":1,"labels":[],"due":{"date":"2026-05-22","string":"yesterday"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--overdue",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Overdue task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_week_uses_next_seven_days_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=next%207%20days&limit=200",
        r#"{"results":[{"id":"week","content":"Week task","priority":1,"labels":[],"due":{"date":"2026-05-29","string":"next week"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--week",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Week task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_upcoming_excludes_today_and_overdue() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=due%20after%3A%20today&limit=200",
        r#"{"results":[{"id":"future","content":"Future task","priority":1,"labels":[],"due":{"date":"2026-05-24","string":"tomorrow"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--upcoming",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Future task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_explicit_filter_overrides_agenda_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=p1&limit=200",
        r#"{"results":[{"id":"p1","content":"Priority task","priority":4,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--today",
            "source:todoist",
            "todoist.filter:p1",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Priority task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_today_todoist_uses_today_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today&limit=200",
        r#"{"results":[{"id":"today","content":"Today shortcut","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "today",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Today shortcut");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_upcoming_todoist_uses_days_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=due%20after%3A%20today%20%26%20next%203%20days&limit=200",
        r#"{"results":[{"id":"soon","content":"Soon shortcut","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "upcoming",
            "--days",
            "3",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Soon shortcut");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_today_all_combines_pkms_and_todoist() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    std::fs::write(
        root.join("roam/common/20260523000002-shortcut-all-today.org"),
        format!(
            r#":PROPERTIES:
:ID:       34343434-3434-4343-8343-343434343434
:END:
#+title: Shortcut All Today
#+filetags: :agenda:

* TODO Local shortcut today
SCHEDULED: <{today}>
"#
        ),
    )
    .unwrap();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today&limit=200",
        r#"{"results":[{"id":"remote-today","content":"Remote shortcut today","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "today",
            "source:all",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = v["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["source"] == "pkms"));
    assert!(items.iter().any(|item| item["source"] == "todoist"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_inbox_defaults_to_todoist_inbox_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%23Inbox&limit=200",
        r#"{"results":[{"id":"inbox","content":"Inbox shortcut","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "inbox",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Inbox shortcut");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_all_today_combines_pkms_and_filtered_todoist() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    std::fs::write(
        root.join("roam/common/20260523000000-today-task.org"),
        format!(
            r#":PROPERTIES:
:ID:       abababab-abab-4aba-abab-abababababab
:END:
#+title: Today Task
#+filetags: :agenda:

* TODO Local today task
SCHEDULED: <{today}>
"#
        ),
    )
    .unwrap();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today&limit=200",
        r#"{"results":[{"id":"remote-today","content":"Remote today task","priority":1,"labels":[],"due":{"date":"2026-05-23","string":"today"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "--today",
            "source:all",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["source"] == "pkms"));
    assert!(items.iter().any(|item| item["source"] == "todoist"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_can_use_config_token() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_with_token(
        vec![(
            "GET",
            "/tasks?limit=200",
            r#"{"results":[{"id":"abc","content":"Config token task","priority":1,"labels":[]}],"next_cursor":null}"#,
        )],
        "config-token",
    );
    let output = run_with_todoist_config_token(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["title"], "Config token task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_show_todoist_uses_stable_remote_id() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/abc",
        r#"{"id":"abc","content":"Remote task","description":"","priority":2,"labels":["remote"]}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "show",
            "todoist:abc",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["source"], "todoist");
    assert_eq!(v["source_id"], "abc");
    assert_eq!(v["title"], "Remote task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_quick_add_uses_mock_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![
        (
            "POST",
            "/tasks/quick",
            r#"{"id":"abc","content":"Buy milk tomorrow"}"#,
        ),
        (
            "GET",
            "/tasks/abc",
            r#"{"id":"abc","content":"Buy milk tomorrow","description":"Get oat milk","project_id":"inbox","priority":2,"labels":["errand"],"due":{"date":"2026-05-24","string":"tomorrow"},"url":"https://todoist.com/showTask?id=abc"}"#,
        ),
        (
            "GET",
            "/projects?limit=200",
            r#"{"results":[{"id":"inbox","name":"Inbox"}],"next_cursor":null}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "--source",
            "todoist",
            "--project",
            "Inbox",
            "Buy milk tomorrow",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["created"], true);
    assert_eq!(
        v["item"]["id"],
        serde_json::json!({"source": "todoist", "id": "abc"})
    );
    assert_eq!(v["item"]["display_id"], "todoist:abc");
    assert_eq!(v["item"]["source"], "todoist");
    assert_eq!(v["item"]["source_id"], "abc");
    assert_eq!(v["item"]["title"], "Buy milk tomorrow");
    assert_eq!(v["item"]["body"], "Get oat milk");
    assert_eq!(v["item"]["status"], "open");
    assert_eq!(v["item"]["state"], "open");
    assert_eq!(v["item"]["priority"], "C");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-05-24");
    assert_eq!(v["item"]["scheduled"]["raw"], "tomorrow");
    assert_eq!(v["item"]["tags"], serde_json::json!(["errand"]));
    assert_eq!(v["item"]["project"], "Inbox");
    assert_eq!(v["item"]["project_id"], "inbox");
    assert_eq!(v["item"]["url"], "https://todoist.com/showTask?id=abc");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_structured_uses_field_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/projects?limit=200",
            serde_json::Value::Null,
            r#"{"results":[{"id":"inbox-id","name":"Inbox"}],"next_cursor":null}"#,
        ),
        (
            "POST",
            "/tasks",
            serde_json::json!({
                "content": "Call Alice",
                "description": "Discuss migration plan",
                "project_id": "inbox-id",
                "labels": ["phone", "migration"],
                "priority": 3,
                "due_date": "2026-05-24",
                "deadline_date": "2026-05-30"
            }),
            r#"{"id":"abc","content":"Call Alice","description":"Discuss migration plan","project_id":"inbox-id","priority":3,"labels":["phone","migration"],"due":{"date":"2026-05-24","string":"2026-05-24"},"deadline":{"date":"2026-05-30"},"url":"https://todoist.com/showTask?id=abc"}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "--source",
            "todoist",
            "--title",
            "Call Alice",
            "--due",
            "2026-05-24",
            "--deadline",
            "2026-05-30",
            "--project",
            "Inbox",
            "--label",
            "phone",
            "--label",
            "migration",
            "--priority",
            "B",
            "--description",
            "Discuss migration plan",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["created"], true);
    assert_eq!(
        v["item"]["id"],
        serde_json::json!({"source": "todoist", "id": "abc"})
    );
    assert_eq!(v["item"]["source"], "todoist");
    assert_eq!(v["item"]["source_id"], "abc");
    assert_eq!(v["item"]["title"], "Call Alice");
    assert_eq!(v["item"]["body"], "Discuss migration plan");
    assert_eq!(v["item"]["priority"], "B");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-05-24");
    assert_eq!(v["item"]["deadline"]["date"], "2026-05-30");
    assert_eq!(v["item"]["tags"], serde_json::json!(["phone", "migration"]));
    assert_eq!(v["item"]["project"], "Inbox");
    assert_eq!(v["item"]["project_id"], "inbox-id");
    assert_eq!(v["item"]["url"], "https://todoist.com/showTask?id=abc");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_text_output_includes_task_fields() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/projects?limit=200",
            serde_json::Value::Null,
            r#"{"results":[{"id":"inbox-id","name":"Inbox"}],"next_cursor":null}"#,
        ),
        (
            "POST",
            "/tasks",
            serde_json::json!({
                "content": "Call Alice",
                "project_id": "inbox-id",
                "priority": 4,
                "due_date": "2026-05-24"
            }),
            r#"{"id":"abc","content":"Call Alice","description":"","project_id":"inbox-id","priority":4,"labels":[],"due":{"date":"2026-05-24","string":"2026-05-24"}}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "add",
            "--source",
            "todoist",
            "--title",
            "Call Alice",
            "--due",
            "2026-05-24",
            "--project",
            "Inbox",
            "--priority",
            "A",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created Todoist task: Call Alice"));
    assert!(stdout.contains("id todoist:abc"));
    assert!(stdout.contains("date 2026-05-24"));
    assert!(stdout.contains("priority A"));
    assert!(stdout.contains("project Inbox"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_duplicate_project_name_fails_before_create() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/projects?limit=200",
        r#"{"results":[{"id":"work-1","name":"Work"},{"id":"work-2","name":"work"}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "--source",
            "todoist",
            "--title",
            "Call Alice",
            "--project",
            "Work",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("matches multiple projects")
    );
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_invalid_structured_priority_fails_before_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "--source",
            "todoist",
            "--title",
            "Call Alice",
            "--priority",
            "urgent",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("Invalid priority"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_invalid_structured_date_fails_before_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "--source",
            "todoist",
            "--title",
            "Call Alice",
            "--due",
            "tomorrow",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("Invalid due date"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_done_todoist_calls_mock_close_endpoint() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![("POST", "/tasks/abc/close", "null")]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "done",
            "todoist:abc",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["source"], "todoist");
    assert_eq!(v["remote_id"], "abc");
    assert_eq!(v["completed"], true);
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_done_todoist_dry_run_does_not_call_mock_api() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "done",
            "todoist:abc",
            "--dry-run",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["completed"], false);
    assert_eq!(v["dry_run"], true);
}

#[cfg(feature = "todoist")]
fn run_with_todoist_env(args: &[&str], base_url: &str) -> std::process::Output {
    let config_home = setup_test_config_home();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    command
        .args(args)
        .env("TODOIST_API_TOKEN", "test-token")
        .env("PKMS_TODOIST_API_BASE_URL", base_url)
        .output()
        .unwrap()
}

#[cfg(feature = "todoist")]
fn run_with_todoist_config_token(args: &[&str], base_url: &str) -> std::process::Output {
    let config_home = tempfile::tempdir().unwrap();
    std::fs::write(
        config_home.path().join("pkms.toml"),
        format!(
            r#"{TEST_CONFIG}

[todoist]
token = "config-token"
"#
        ),
    )
    .unwrap();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    command
        .args(args)
        .env_remove("TODOIST_API_TOKEN")
        .env("PKMS_TODOIST_API_BASE_URL", base_url)
        .output()
        .unwrap()
}

#[cfg(feature = "todoist")]
fn spawn_todoist_mock(
    responses: Vec<(&'static str, &'static str, &'static str)>,
) -> (String, thread::JoinHandle<()>) {
    spawn_todoist_mock_with_token(responses, "test-token")
}

#[cfg(feature = "todoist")]
fn spawn_todoist_mock_expect_bodies(
    responses: Vec<(&'static str, &'static str, serde_json::Value, &'static str)>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, expected_body, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(1)))
                .unwrap();
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request_bytes.extend_from_slice(&buffer[..read]);
                let request = String::from_utf8_lossy(&request_bytes);
                if let Some((headers, body)) = request.split_once("\r\n\r\n") {
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&request_bytes);
            assert!(
                request.starts_with(&format!("{method} {expected_path} HTTP/1.1")),
                "unexpected request: {request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-token")
            );
            if !expected_body.is_null() {
                let (_, request_body) = request
                    .split_once("\r\n\r\n")
                    .expect("expected request body separator");
                let actual_body: serde_json::Value = serde_json::from_str(request_body).unwrap();
                assert_eq!(actual_body, expected_body);
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (base_url, handle)
}

#[cfg(feature = "todoist")]
fn spawn_todoist_mock_with_token(
    responses: Vec<(&'static str, &'static str, &'static str)>,
    expected_token: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(
                request.starts_with(&format!("{method} {expected_path} HTTP/1.1")),
                "unexpected request: {request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {expected_token}"))
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (base_url, handle)
}
