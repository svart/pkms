use super::*;
#[cfg(feature = "todoist")]
use std::io::{Read, Write};
#[cfg(feature = "todoist")]
use std::net::TcpListener;
#[cfg(feature = "todoist")]
use std::process::Command;
#[cfg(feature = "todoist")]
use std::thread;

#[test]
fn test_task_help_lists_subcommands() {
    let (stdout, stderr, status) = run(&["task", "--help"]);
    assert!(status.success(), "task --help failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("list"));
    assert!(stdout.contains("agenda"));
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
fn test_task_agenda_all_today_combines_pkms_and_filtered_todoist() {
    let (_dir, root) = setup_db();
    std::fs::write(
        root.join("roam/common/20260523000000-today-task.org"),
        r#":PROPERTIES:
:ID:       abababab-abab-4aba-abab-abababababab
:END:
#+title: Today Task
#+filetags: :agenda:

* TODO Local today task
SCHEDULED: <2026-05-23 Sat>
"#,
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
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "POST",
        "/tasks/quick",
        r#"{"id":"abc","content":"Buy milk tomorrow"}"#,
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
            "--project",
            "Inbox",
            "Buy milk tomorrow",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["source"], "todoist");
    assert_eq!(v["remote_id"], "abc");
    assert_eq!(v["created"], true);
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
