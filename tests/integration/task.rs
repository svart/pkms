use super::*;
use chrono::{Datelike, Weekday};
#[cfg(feature = "todoist")]
use std::io::{Read, Write};
#[cfg(feature = "todoist")]
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "todoist")]
use std::process::{Command, Output, Stdio};
#[cfg(feature = "todoist")]
use std::thread;
#[cfg(feature = "todoist")]
use std::time::{Duration, Instant};

#[cfg(feature = "todoist")]
const TODOIST_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(feature = "todoist")]
const TODOIST_MOCK_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "todoist")]
const TODOIST_MOCK_READ_TIMEOUT: Duration = Duration::from_secs(2);

fn org_date(days_from_today: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(days_from_today))
        .format("%Y-%m-%d")
        .to_string()
}

fn upcoming_weekday_date(weekday: Weekday) -> String {
    let today = chrono::Local::now().date_naive();
    let today_index = today.weekday().num_days_from_monday() as i64;
    let target_index = weekday.num_days_from_monday() as i64;
    let mut days_until = (target_index - today_index).rem_euclid(7);
    if days_until == 0 {
        days_until = 7;
    }
    (today + chrono::Duration::days(days_until))
        .format("%Y-%m-%d")
        .to_string()
}

fn section_box_start(title: &str) -> String {
    format!("╭─── {title}")
}

#[test]
fn test_task_help_lists_subcommands() {
    let (stdout, stderr, status) = run(&["task", "--help"]);
    assert!(status.success(), "task --help failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("list"));
    assert!(stdout.contains("agenda"));
    assert!(stdout.contains("inbox"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("open"));
    assert!(stdout.contains("state"));
    assert!(stdout.contains("done"));
    assert!(stdout.contains("add"));
    assert!(stdout.contains("mod dep:<PARENT-ID>"));
    assert!(stdout.contains("Task modifiers apply to `task add` and `task <ID> mod`."));
    assert!(stdout.contains("title:<text>"));
    assert!(stdout.contains("state:<state>"));
    assert!(stdout.contains("sch:<date>"));
    assert!(stdout.contains("dead:<date>"));
    assert!(stdout.contains("note:<uuid-title-or-path>"));
    assert!(stdout.contains("dep:<task-id>"));
    assert!(!stdout.contains("report"));
    assert!(!stdout.contains("plan"));
    let commands = task_help_commands(&stdout);
    assert!(!commands.contains(&"projects"));
    assert!(!commands.contains(&"labels"));
    assert!(!commands.contains(&"clarify"));
    assert!(!commands.contains(&"update"));
    assert!(!commands.contains(&"delete"));
    assert!(!commands.contains(&"reopen"));
}

#[test]
fn test_removed_task_subcommands_are_rejected() {
    for subcommand in [
        "today", "overdue", "upcoming", "projects", "labels", "clarify", "update", "delete",
        "reopen",
    ] {
        let (stdout, stderr, status) = run(&["task", subcommand, "--help"]);
        assert!(
            !status.success(),
            "task {subcommand} unexpectedly succeeded:\n{stdout}\n{stderr}"
        );
    }
}

#[test]
fn test_removed_top_level_task_commands_are_rejected() {
    for command in ["todo", "agenda", "show", "open"] {
        let (stdout, stderr, status) = run(&[command, "--help"]);
        assert!(
            !status.success(),
            "pkms {command} unexpectedly succeeded:\n{stdout}\n{stderr}"
        );
    }
}

#[test]
fn test_task_agenda_help_lists_shortcut_subcommands() {
    let (stdout, stderr, status) = run(&["task", "agenda", "--help"]);
    assert!(
        status.success(),
        "task agenda --help failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("today"));
    assert!(stdout.contains("week"));
    assert!(stdout.contains("overdue"));
    assert!(stdout.contains("upcoming"));
    assert!(!stdout.contains("--today"));
    assert!(!stdout.contains("--week"));
    assert!(!stdout.contains("--overdue"));
    assert!(!stdout.contains("--upcoming"));
}

#[test]
fn test_task_list_help_shows_filters() {
    let (stdout, stderr, status) = run(&["task", "list", "--help"]);
    assert!(
        status.success(),
        "task list --help failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("Filters:"));
    assert!(stdout.contains("source:pkms|todoist|all"));
    assert!(stdout.contains("todoist.filter:<query>"));
}

#[test]
fn test_task_add_help_shows_modifiers() {
    let (stdout, stderr, status) = run(&["task", "add", "--help"]);
    assert!(
        status.success(),
        "task add --help failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("Add modifiers:"));
    assert!(stdout.contains("state:<state>"));
    assert!(stdout.contains("sch:<date>"));
    assert!(stdout.contains("dead:<date>"));
    assert!(stdout.contains("note:<uuid-title-or-path> PKMS only"));
    assert!(stdout.contains("dep:<task-id>"));
    assert!(!stdout.contains("--title"));
    assert!(!stdout.contains("--source"));
}

#[test]
fn test_task_agenda_shortcut_flags_are_rejected() {
    for flag in ["--today", "--week", "--overdue", "--upcoming"] {
        let (stdout, stderr, status) = run(&["task", "agenda", flag, "--help"]);
        assert!(
            !status.success(),
            "task agenda {flag} unexpectedly succeeded:\n{stdout}\n{stderr}"
        );
    }
}

fn task_help_commands(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.strip_prefix("  ")?;
            let command = line.split_whitespace().next()?;
            (command != "-h," && command != "--db" && command != "--output-format")
                .then_some(command)
        })
        .collect()
}

#[test]
fn test_task_list_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some());
    assert!(v["items"].as_array().is_some_and(|items| !items.is_empty()));
}

#[test]
fn test_task_list_default_json_uses_source_neutral_items() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Task Note",
            "11111111-1111-4111-8111-111111111111",
        )
        .task("tasks.org", "TODO", "Default JSON task");

    let (v, status) = db.run_json(&["task", "list"]);

    assert!(status.success());
    assert_eq!(v["total"], 1);
    assert_eq!(v["items"][0]["source"], "pkms");
    assert_eq!(v["items"][0]["source_id"], "1");
    assert_eq!(v["items"][0]["display_id"], "p1");
    assert_eq!(v["items"][0]["title"], "Default JSON task");
    assert_eq!(v["items"][0]["note_title"], "Task Note");
    assert_eq!(v["items"][0]["state"], "TODO");
    assert!(v["items"][0].get("todo_state").is_none());
}

#[test]
fn test_task_list_text() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "task", "list"]);
    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Total:"));
    assert!(stdout.contains("High priority task"));
}

#[test]
fn test_task_list_formats_inline_markup_when_terminal_formatting_is_forced() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Task Note",
            "11111111-1111-4111-8111-111111111111",
        )
        .task(
            "tasks.org",
            "TODO",
            "Use =literal @skip <tag>= and ~orange @skip~ for @alice email@example.com @",
        );

    let (plain_stdout, plain_stderr, plain_status) =
        db.run(&["task", "list", "--columns=id,heading"]);
    assert!(
        plain_status.success(),
        "plain task list failed:\n{plain_stdout}\n{plain_stderr}"
    );
    assert!(plain_stdout.contains("=literal @skip <tag>="));
    assert!(plain_stdout.contains("~orange @skip~"));
    assert!(!plain_stdout.contains("\x1b["));

    let config_home = setup_test_config_home();
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .args([
            "--db",
            db.root().to_str().unwrap(),
            "task",
            "list",
            "--columns=id,heading",
        ])
        .env("CLICOLOR_FORCE", "1")
        .env("COLUMNS", "120")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "forced task list failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("\x1b[2mliteral @skip <tag>\x1b[0m"));
    assert!(stdout.contains("\x1b[38;5;166morange @skip\x1b[0m"));
    assert!(stdout.contains("\x1b[38;5;39m@alice\x1b[0m"));
    assert!(stdout.contains("email@example.com @"));
    assert!(!stdout.contains("=literal @skip <tag>="));
    assert!(!stdout.contains("~orange @skip~"));
}

#[test]
fn test_task_list_limit_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--limit",
        "1",
    ]);

    assert!(status.success());
    assert!(v["items"].as_array().unwrap().len() <= 1);
}

#[test]
fn test_task_agenda_limit_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--limit",
        "1",
    ]);

    assert!(status.success());
    assert!(v["items"].as_array().unwrap().len() <= 1);
    assert!(v["total"].as_u64().unwrap_or(0) >= v["items"].as_array().unwrap().len() as u64);
}

#[test]
fn test_task_list_group_state_json() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--group",
        "state",
    ]);
    assert!(task_status.success());
    assert_eq!(task["group_field"], "state");
    let groups = task["groups"].as_object().unwrap();
    assert!(groups.contains_key("TODO"));
    assert!(groups.contains_key("DONE"));
    let first_todo = groups["TODO"].as_array().unwrap().first().unwrap();
    assert_eq!(first_todo["source"], "pkms");
    assert!(first_todo.get("display_id").is_some());
    assert!(first_todo.get("todo_state").is_none());
}

#[test]
fn test_task_list_group_text_uses_boxed_section_delimiters() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "--group",
        "state",
    ]);

    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains(&section_box_start("TODO (")),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains('╰'), "stdout:\n{stdout}");
    assert!(!stdout.contains("=== TODO"), "stdout:\n{stdout}");
}

#[test]
fn test_task_list_rejects_unknown_group_field() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--group",
        "unknown",
    ]);

    assert!(!status.success());
    let v = assert_json_error_output(&["task", "list", "--group", "unknown"], &stdout);
    assert_eq!(
        v["error"],
        "Unknown task group field 'unknown'. Use state, file, or priority."
    );
}

#[test]
fn test_task_list_from_stdin_scopes_to_resolved_note() {
    let (_dir, root) = setup_db();
    let (task_stdout, task_stderr, task_status) = run_pipe(
        &[
            "--db",
            root.to_str().unwrap(),
            "resolve",
            "--title",
            "Agenda Item",
            "--output-format",
            "ndjson",
        ],
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "--from-stdin",
        ],
    );
    assert!(
        task_status.success(),
        "task list --from-stdin failed:\n{task_stdout}\n{task_stderr}"
    );
    let task: serde_json::Value = serde_json::from_str(task_stdout.trim()).unwrap();
    let items = task["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|item| {
        item["source"].as_str() == Some("pkms")
            && item["note_title"].as_str() == Some("Agenda Item")
            && item["title"]
                .as_str()
                .is_some_and(|title| title != "Agenda Item")
            && item.get("todo_state").is_none()
    }));
}

#[test]
fn test_task_list_pkms_text_uses_bare_source_ids() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "--sort",
        "priority",
        "--limit",
        "1",
    ]);

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let row = stdout
        .lines()
        .find(|line| line.contains("High priority task"))
        .expect("expected a task row");
    let id = row.split_whitespace().next().unwrap();
    assert!(
        id.parse::<usize>().is_ok(),
        "expected bare numeric id: {id}"
    );
}

#[test]
fn test_task_agenda_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some());
    assert!(v["items"].as_array().is_some_and(|items| !items.is_empty()));
}

#[test]
fn test_task_agenda_default_json_uses_source_neutral_items() {
    let db = TestDb::new().note_with_content(
        "planned.org",
        r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: Planned Note

* TODO Default agenda task
SCHEDULED: <2026-05-29 Fri>
"#,
    );

    let (v, status) = db.run_json(&["task", "agenda"]);

    assert!(status.success());
    assert_eq!(v["total"], 1);
    assert_eq!(v["items"][0]["source"], "pkms");
    assert_eq!(v["items"][0]["source_id"], "1");
    assert_eq!(v["items"][0]["display_id"], "p1");
    assert_eq!(v["items"][0]["title"], "Default agenda task");
    assert_eq!(v["items"][0]["note_title"], "Planned Note");
    assert_eq!(v["items"][0]["scheduled"]["date"], "2026-05-29");
    assert!(v["items"][0].get("scheduled_date").is_none());
}

#[test]
fn test_task_agenda_text() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&["--db", root.to_str().unwrap(), "task", "agenda"]);
    assert!(status.success(), "task agenda failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("High priority task"));
    assert!(stdout.contains("task(s)"));
}

#[test]
fn test_task_agenda_text_groups_daily_task_by_explicit_schedule() {
    let db = TestDb::clean();
    let today = org_date(0);
    let scheduled = upcoming_weekday_date(Weekday::Wed);
    db.write_roam(
        &format!("{today}.org"),
        &format!(
            r#":PROPERTIES:
:ID:       12121212-1212-4121-8121-121212121212
:END:
#+title: Daily Schedule Override
#+filetags: :daily:

* TODO Scheduled away from daily note
SCHEDULED: <{scheduled}>
"#
        ),
    );

    let (stdout, stderr, status) = db.run(&["task", "agenda"]);
    assert!(status.success(), "task agenda failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains(&section_box_start("Upcoming")),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains('╰'), "stdout:\n{stdout}");
    assert!(!stdout.contains("=== Upcoming ==="), "stdout:\n{stdout}");
    assert!(
        !stdout.contains(&section_box_start("Today")),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains(&scheduled), "stdout:\n{stdout}");

    let (stdout, stderr, status) = db.run(&["task", "agenda", "state:TODO"]);
    assert!(
        status.success(),
        "filtered task agenda failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(&section_box_start("Upcoming")),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains('╰'), "stdout:\n{stdout}");
    assert!(!stdout.contains("=== Upcoming ==="), "stdout:\n{stdout}");
    assert!(
        !stdout.contains(&section_box_start("Today")),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains(&scheduled), "stdout:\n{stdout}");

    let (v, status) = db.run_json(&["task", "agenda", "date:today"]);
    assert!(status.success());
    assert_eq!(v["total"], 0, "json:\n{v:#}");

    let scheduled_filter = format!("date:{scheduled}");
    let (v, status) = db.run_json(&["task", "agenda", &scheduled_filter]);
    assert!(status.success());
    assert_eq!(task_titles(&v), vec!["Scheduled away from daily note"]);
}

#[test]
fn test_task_agenda_days_filters_window_json() {
    let db = TestDb::clean();
    let overdue = org_date(-1);
    let today = org_date(0);
    let tomorrow = org_date(1);
    let day_after = org_date(2);
    let later = org_date(3);
    db.write_roam(
        "days-window.org",
        &format!(
            r#":PROPERTIES:
:ID:       56565656-5656-4656-8656-565656565656
:END:
#+title: Days Window
#+filetags: :agenda:

* TODO Overdue window task
SCHEDULED: <{overdue}>
* TODO Today window task
SCHEDULED: <{today}>
* TODO Tomorrow window task
SCHEDULED: <{tomorrow}>
* TODO Day after window task
SCHEDULED: <{day_after}>
* TODO Later window task
SCHEDULED: <{later}>
"#
        ),
    );

    let (v, status) = db.run_json(&["task", "agenda", "--days", "3"]);

    assert!(status.success());
    assert_eq!(v["total"], 4);
    let titles = task_titles(&v);
    assert!(titles.contains(&"Overdue window task".to_string()));
    assert!(titles.contains(&"Today window task".to_string()));
    assert!(titles.contains(&"Tomorrow window task".to_string()));
    assert!(titles.contains(&"Day after window task".to_string()));
    assert!(!titles.contains(&"Later window task".to_string()));

    let (filtered, filtered_status) = db.run_json(&["task", "agenda", "--days", "3", "state:TODO"]);
    assert!(filtered_status.success());
    assert_eq!(filtered["total"], 4);
    assert!(!task_titles(&filtered).contains(&"Later window task".to_string()));
}

#[test]
fn test_task_agenda_days_text_splits_each_day() {
    let db = TestDb::clean();
    let overdue = org_date(-1);
    let today = org_date(0);
    let tomorrow = org_date(1);
    let day_after_date = chrono::Local::now().date_naive() + chrono::Duration::days(2);
    let day_after = day_after_date.format("%Y-%m-%d").to_string();
    let later = org_date(3);
    db.write_roam(
        "days-sections.org",
        &format!(
            r#":PROPERTIES:
:ID:       67676767-6767-4676-8676-676767676767
:END:
#+title: Days Sections
#+filetags: :agenda:

* TODO Overdue section task
SCHEDULED: <{overdue}>
* TODO Today section task
SCHEDULED: <{today}>
* TODO Tomorrow section task
SCHEDULED: <{tomorrow}>
* TODO Day after section task
SCHEDULED: <{day_after}>
* TODO Later section task
SCHEDULED: <{later}>
"#
        ),
    );

    let (stdout, stderr, status) =
        db.run(&["task", "agenda", "--days", "3", "--columns=date,heading"]);

    assert!(
        status.success(),
        "task agenda --days failed:\n{stdout}\n{stderr}"
    );
    let day_after_header = section_box_start(&day_after_date.format("%Y-%m-%d %a").to_string());
    let overdue_idx = stdout
        .find(&section_box_start("Overdue"))
        .expect("overdue section");
    let today_idx = stdout
        .find(&section_box_start("Today"))
        .expect("today section");
    let tomorrow_idx = stdout
        .find(&section_box_start("Tomorrow"))
        .expect("tomorrow section");
    let day_after_idx = stdout.find(&day_after_header).expect("day-after section");
    assert!(overdue_idx < today_idx);
    assert!(today_idx < tomorrow_idx);
    assert!(tomorrow_idx < day_after_idx);
    assert!(stdout.contains('╰'), "stdout:\n{stdout}");
    assert!(!stdout.contains("=== Overdue ==="), "stdout:\n{stdout}");
    assert!(stdout.contains("Overdue section task"), "stdout:\n{stdout}");
    assert!(stdout.contains("Today section task"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("Tomorrow section task"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Day after section task"),
        "stdout:\n{stdout}"
    );
    assert!(!stdout.contains("Later section task"), "stdout:\n{stdout}");
}

#[test]
fn test_task_list_daily_note_date_includes_weekday_without_planning_markers() {
    let db = TestDb::new().note_with_content(
        "2024-06-15.org",
        r#":PROPERTIES:
:ID:       15151515-1515-4515-8515-151515151515
:END:
#+title: Daily Tasks
#+filetags: :daily:

* TODO Daily fallback date
"#,
    );

    let (stdout, stderr, status) = db.run(&["task", "list", "--columns=id,date,type,heading"]);

    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("2024-06-15 Sat"), "stdout:\n{stdout}");
    assert!(stdout.contains("Daily fallback date"), "stdout:\n{stdout}");
}

#[test]
fn test_task_agenda_today_returns_source_neutral_json() {
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
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "today",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|item| { item["source"] == "pkms" && item["title"] == "Shortcut today task" })
    );
}

#[test]
fn test_task_agenda_week_returns_source_neutral_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "week",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items.iter().all(|item| item["source"] == "pkms"));
}

#[test]
fn test_task_agenda_shortcuts_match_date_filters() {
    let (_dir, root) = setup_clean_db();
    let yesterday = org_date(-1);
    let today = org_date(0);
    let upcoming = org_date(2);
    let later = org_date(9);
    db_write(
        &root,
        "common/20260630000000-agenda-shortcut-aliases.org",
        &format!(
            r#":PROPERTIES:
:ID:       acacacac-acac-4aca-8cac-acacacacacac
:END:
#+title: Agenda Shortcut Aliases
#+filetags: :agenda:

* TODO Alias overdue task
SCHEDULED: <{yesterday}>
* TODO Alias today task
SCHEDULED: <{today}>
* TODO Alias upcoming task
SCHEDULED: <{upcoming}>
* TODO Alias later task
SCHEDULED: <{later}>
"#
        ),
    );

    let root = root.to_str().unwrap();
    for (shortcut, date_filter) in [
        ("today", "date:today"),
        ("week", "date:week"),
        ("overdue", "date:overdue"),
        ("upcoming", "date:upcoming"),
    ] {
        let date_args = ["--db", root, "task", "agenda", date_filter];
        let shortcut_args = ["--db", root, "task", "agenda", shortcut];
        let (expected_stdout, expected_stderr, expected_status) = run(&date_args);
        let (actual_stdout, actual_stderr, actual_status) = run(&shortcut_args);
        assert!(
            expected_status.success(),
            "date filter failed for {date_filter}:\n{expected_stdout}\n{expected_stderr}"
        );
        assert!(
            actual_status.success(),
            "shortcut failed for {shortcut}:\n{actual_stdout}\n{actual_stderr}"
        );
        assert_eq!(actual_stdout, expected_stdout, "shortcut {shortcut}");
        assert_eq!(actual_stderr, expected_stderr, "shortcut {shortcut}");

        let date_json_args = [
            "--db",
            root,
            "--output-format",
            "json",
            "task",
            "agenda",
            date_filter,
        ];
        let shortcut_json_args = [
            "--db",
            root,
            "--output-format",
            "json",
            "task",
            "agenda",
            shortcut,
        ];
        let (expected_json, expected_status) = run_json(&date_json_args);
        let (actual_json, actual_status) = run_json(&shortcut_json_args);
        assert!(
            expected_status.success(),
            "date filter failed for {date_filter}"
        );
        assert!(actual_status.success(), "shortcut failed for {shortcut}");
        assert_eq!(actual_json, expected_json, "shortcut {shortcut}");
    }
}

#[test]
fn test_task_agenda_week_honors_columns() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "agenda",
        "week",
        "--columns=id,date,state,prio,tags,heading",
    ]);

    assert!(
        status.success(),
        "task agenda week --columns failed:\n{stdout}\n{stderr}"
    );
    let header = stdout.lines().next().unwrap_or_default();
    assert!(header.contains("Id"), "stdout:\n{stdout}");
    assert!(header.contains("Date"), "stdout:\n{stdout}");
    assert!(header.contains("State"), "stdout:\n{stdout}");
    assert!(header.contains("Prio"), "stdout:\n{stdout}");
    assert!(header.contains("Tags"), "stdout:\n{stdout}");
    assert!(header.contains("Heading"), "stdout:\n{stdout}");
    assert!(!header.contains("Type"), "stdout:\n{stdout}");
    assert!(!header.contains("Project"), "stdout:\n{stdout}");
    assert!(!header.contains("Note"), "stdout:\n{stdout}");
}

#[test]
fn test_task_list_uses_pkms_task_columns_config() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run_with_config(
        &["--db", root.to_str().unwrap(), "task", "list", "prio:none"],
        r#"
[columns.pkms]
tasks = ["Id", "Heading"]
"#,
    );

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let header = stdout.lines().next().unwrap_or_default();
    assert!(header.contains("Id"), "stdout:\n{stdout}");
    assert!(header.contains("Heading"), "stdout:\n{stdout}");
    assert!(!header.contains("Date"), "stdout:\n{stdout}");
    assert!(!header.contains("Project"), "stdout:\n{stdout}");
}

#[test]
fn test_task_agenda_uses_pkms_agenda_columns_config() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run_with_config(
        &["--db", root.to_str().unwrap(), "task", "agenda", "week"],
        r#"
[columns.pkms]
agenda = ["Id", "Date", "Heading"]
"#,
    );

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let header = stdout.lines().next().unwrap_or_default();
    assert!(header.contains("Id"), "stdout:\n{stdout}");
    assert!(header.contains("Date"), "stdout:\n{stdout}");
    assert!(header.contains("Heading"), "stdout:\n{stdout}");
    assert!(!header.contains("Project"), "stdout:\n{stdout}");
    assert!(!header.contains("Note"), "stdout:\n{stdout}");
}

#[test]
fn test_task_columns_can_adjust_current_default_set() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "list",
            "prio:none",
            "--columns=+project",
        ],
        r#"
[columns.pkms]
tasks = ["Id", "Heading"]
"#,
    );

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let header = stdout.lines().next().unwrap_or_default();
    assert!(header.contains("Id"), "stdout:\n{stdout}");
    assert!(header.contains("Project"), "stdout:\n{stdout}");
    assert!(header.contains("Heading"), "stdout:\n{stdout}");

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "list",
            "prio:none",
            "--columns=-project",
        ],
        r#"
[columns.pkms]
tasks = ["Id", "Project", "Heading"]
"#,
    );

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let header = stdout.lines().next().unwrap_or_default();
    assert!(header.contains("Id"), "stdout:\n{stdout}");
    assert!(!header.contains("Project"), "stdout:\n{stdout}");
    assert!(header.contains("Heading"), "stdout:\n{stdout}");
}

#[test]
fn test_task_columns_report_ambiguous_or_invalid_columns() {
    let (_dir, root) = setup_db();
    let (_stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "prio:none",
        "--columns=+project,heading",
    ]);
    assert!(!status.success());
    assert!(stderr.contains("cannot mix"), "stderr:\n{stderr}");

    let (_stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "prio:none",
        "--columns=unknown",
    ]);
    assert!(!status.success());
    assert!(stderr.contains("Unknown column"), "stderr:\n{stderr}");
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
        "agenda",
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
        "agenda",
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
        "agenda",
        "overdue",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert!(v["items"].as_array().unwrap().len() <= 1);
}

#[test]
fn test_task_list_projects_pkms_uses_project_properties() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "projects",
        "source:pkms",
    ]);
    assert!(status.success());
    assert_metadata_row(&v, "pkms", "Heading Project");
    assert_metadata_row(&v, "pkms", "Note Project");
}

#[test]
fn test_task_list_tags_pkms_uses_filetags_and_heading_tags() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "tags",
        "source:pkms",
    ]);
    assert!(status.success());
    assert_metadata_row(&v, "pkms", "filetag");
    assert_metadata_row(&v, "pkms", "headingtag");
}

#[test]
fn test_task_list_accepts_state_tags_type_and_prio_filters() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "state:TODO,!WAITING",
        "tags:project",
        "type:SCHED",
        "prio:none",
    ]);

    assert!(status.success());
    let titles = task_titles(&v);
    assert!(titles.contains(&"Parent task".to_string()));
    assert!(!titles.contains(&"Child task A".to_string()));
    assert!(!titles.contains(&"High priority task".to_string()));
}

#[test]
fn test_task_list_state_meta_filters_use_configured_states() {
    let db = TestDb::new().note_with_content(
        "state-meta.org",
        r#":PROPERTIES:
:ID:       56565656-5656-4656-8656-565656565656
:END:
#+title: State Meta Filters

* NEXT Next task
* BLOCKED Blocked task
* DONE Done task
* DROPPED Dropped task
"#,
    );
    let config = r#"[agenda]
open_todo_states = ["NEXT", "BLOCKED"]
closed_todo_states = ["DONE", "DROPPED"]
"#;

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            db.root().to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "state:opened,!blocked",
        ],
        config,
    );
    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(task_titles(&v), vec!["Next task"]);

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            db.root().to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "state:!closed",
        ],
        config,
    );
    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let titles = task_titles(&v);
    assert!(titles.contains(&"Next task".to_string()));
    assert!(titles.contains(&"Blocked task".to_string()));
    assert!(!titles.contains(&"Done task".to_string()));
    assert!(!titles.contains(&"Dropped task".to_string()));
}

#[test]
fn test_task_list_accepts_comma_separated_prio_filter() {
    let (_dir, root) = setup_db();
    std::fs::write(
        root.join("roam/common/20260525000000-priority_filters.org"),
        r#":PROPERTIES:
:ID:       88888888-8888-4888-8888-888888888888
:END:
#+title: Priority Filter Tasks

* TODO [#A] Priority A task
* TODO [#B] Priority B task
* TODO [#C] Priority C task
* TODO No priority task
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "prio:a,b,c",
    ]);

    assert!(status.success());
    let titles = task_titles(&v);
    assert!(titles.contains(&"Priority A task".to_string()));
    assert!(titles.contains(&"Priority B task".to_string()));
    assert!(titles.contains(&"Priority C task".to_string()));
    assert!(titles.contains(&"High priority task".to_string()));
    assert!(!titles.contains(&"No priority task".to_string()));
    assert!(!titles.contains(&"Low priority task".to_string()));
}

#[test]
fn test_task_list_accepts_project_filter() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "project:Heading Project",
    ]);

    assert!(status.success());
    assert_eq!(task_titles(&v), vec!["Heading project task"]);
}

#[test]
fn test_task_list_accepts_project_exclusion_filter() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "project:!Heading Project",
    ]);

    assert!(status.success());
    let titles = task_titles(&v);
    assert!(titles.contains(&"Note project task".to_string()));
    assert!(!titles.contains(&"Heading project task".to_string()));
}

#[test]
fn test_task_list_accepts_tag_alias_and_exclusion_filter() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "tag:project,!agenda",
    ]);

    assert!(status.success());
    let titles = task_titles(&v);
    assert!(titles.contains(&"Parent task".to_string()));
    assert!(!titles.contains(&"High priority task".to_string()));
}

#[test]
fn test_task_list_accepts_scope_after_and_before_filters() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "scope:Nested Todo Note",
        "after:2026-06-01",
        "before:2026-06-05",
    ]);

    assert!(status.success());
    assert_eq!(task_titles(&v), vec!["Parent task"]);
}

#[test]
fn test_task_list_source_neutral_sort_supports_file_sort_field() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "state:TODO",
        "--sort",
        "file",
    ]);

    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(items[0]["note_title"].as_str(), Some("Agenda Item"));
}

#[test]
fn test_task_list_pkms_default_rejects_unknown_sort_field() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--sort",
        "unknown",
    ]);

    assert!(!status.success());
    let v = assert_json_error_output(&["task", "list", "--sort", "unknown"], &stdout);
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Unknown task sort field 'unknown'")
    );
}

#[test]
fn test_task_list_rejects_unknown_sort_field() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "state:TODO",
        "--sort",
        "unknown",
    ]);

    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Unknown task sort field 'unknown'")
    );
}

#[test]
fn test_task_list_pkms_default_rejects_empty_sort_field() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--sort",
        "",
    ]);

    assert!(!status.success());
    let v = assert_json_error_output(&["task", "list", "--sort", ""], &stdout);
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Task sort must include at least one field")
    );
}

#[test]
fn test_task_list_pkms_default_sorts_by_task_title() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Sort Tasks",
            "44444444-4444-4444-8444-444444444444",
        )
        .task("tasks.org", "TODO", "Beta task")
        .task("tasks.org", "TODO", "Alpha task");

    let (v, status) = db.run_json(&["task", "list", "--sort", "title"]);

    assert!(status.success());
    assert_eq!(task_titles(&v), vec!["Alpha task", "Beta task"]);
}

#[test]
fn test_task_list_pkms_default_sorts_by_date_then_priority() {
    let db = task_default_sort_db();

    let (v, status) = db.run_json(&["task", "list"]);

    assert!(status.success());
    assert_eq!(
        task_titles(&v),
        vec![
            "Same day medium task",
            "Earlier low task",
            "Later urgent task"
        ]
    );
}

#[test]
fn test_task_list_source_neutral_default_sorts_by_date_then_priority() {
    let db = task_default_sort_db();

    let (v, status) = db.run_json(&["task", "list", "state:TODO"]);

    assert!(status.success());
    assert_eq!(
        task_titles(&v),
        vec![
            "Same day medium task",
            "Earlier low task",
            "Later urgent task"
        ]
    );
}

#[test]
fn test_task_list_group_sorts_tasks_inside_each_group() {
    let db = task_default_sort_db();

    let (v, status) = db.run_json(&["task", "list", "--group", "state"]);

    assert!(status.success());
    let todo_titles: Vec<_> = v["groups"]["TODO"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert_eq!(
        todo_titles,
        vec![
            "Same day medium task",
            "Earlier low task",
            "Later urgent task"
        ]
    );
}

#[test]
fn test_task_agenda_source_neutral_sort_supports_date_sort_fields() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "state:TODO",
        "--sort",
        "scheduled,deadline,file",
    ]);

    assert!(status.success());
    assert!(v["items"].is_array());
}

#[test]
fn test_task_agenda_pkms_default_rejects_unknown_sort_field() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--sort",
        "unknown",
    ]);

    assert!(!status.success());
    let v = assert_json_error_output(&["task", "agenda", "--sort", "unknown"], &stdout);
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Unknown task sort field 'unknown'")
    );
}

#[test]
fn test_task_agenda_accepts_date_filters() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let upcoming = org_date(2);
    std::fs::write(
        root.join("roam/common/20260525000000-filter-today.org"),
        format!(
            r#":PROPERTIES:
:ID:       57575757-5757-4757-8757-575757575757
:END:
#+title: Filter Today
#+filetags: :agenda:

* TODO Filter today task
SCHEDULED: <{today}>
* TODO Filter upcoming task
SCHEDULED: <{upcoming}>
"#
        ),
    )
    .unwrap();

    let (today_tasks, today_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "date:today",
    ]);
    assert!(today_status.success());
    assert!(task_titles(&today_tasks).contains(&"Filter today task".to_string()));

    let (upcoming_tasks, upcoming_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "date:upcoming",
    ]);
    assert!(upcoming_status.success());
    let upcoming_titles = task_titles(&upcoming_tasks);
    assert!(upcoming_titles.contains(&"Filter upcoming task".to_string()));
    assert!(!upcoming_titles.contains(&"Filter today task".to_string()));

    let (week_tasks, week_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "date:week",
    ]);
    assert!(week_status.success());
    assert!(task_titles(&week_tasks).contains(&"Filter upcoming task".to_string()));
}

#[test]
fn test_task_agenda_accepts_exact_and_bare_date_filters() {
    let (_dir, root) = setup_db();
    let upcoming = org_date(2);
    std::fs::write(
        root.join("roam/common/20260525000000-filter-upcoming.org"),
        format!(
            r#":PROPERTIES:
:ID:       57575757-5757-4757-8757-575757575758
:END:
#+title: Filter Upcoming
#+filetags: :agenda:

* TODO Filter upcoming task
SCHEDULED: <{upcoming}>
"#
        ),
    )
    .unwrap();

    let (exact_tasks, exact_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "date:2026-05-10",
    ]);
    assert!(exact_status.success());
    let exact_titles = task_titles(&exact_tasks);
    assert!(exact_titles.contains(&"High priority task".to_string()));
    assert!(exact_titles.contains(&"Fix this".to_string()));
    assert!(!exact_titles.contains(&"Parent task".to_string()));

    let (overdue_tasks, overdue_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "overdue",
    ]);
    assert!(overdue_status.success());
    assert!(task_titles(&overdue_tasks).contains(&"High priority task".to_string()));

    let (upcoming_tasks, upcoming_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "upcoming",
    ]);
    assert!(upcoming_status.success());
    assert!(task_titles(&upcoming_tasks).contains(&"Filter upcoming task".to_string()));
}

#[test]
fn test_task_agenda_accepts_comma_separated_date_filters() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let upcoming = org_date(2);
    std::fs::write(
        root.join("roam/common/20260525000000-filter-today.org"),
        format!(
            r#":PROPERTIES:
:ID:       57575757-5757-4757-8757-575757575757
:END:
#+title: Filter Today
#+filetags: :agenda:

* TODO Filter today task
SCHEDULED: <{today}>
* TODO Filter upcoming task
SCHEDULED: <{upcoming}>
"#
        ),
    )
    .unwrap();

    let (tasks, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "date:today,2026-05-10",
    ]);
    assert!(status.success());
    let titles = task_titles(&tasks);
    assert!(titles.contains(&"Filter today task".to_string()));
    assert!(titles.contains(&"High priority task".to_string()));
    assert!(titles.contains(&"Fix this".to_string()));
    assert!(!titles.contains(&"Filter upcoming task".to_string()));
}

#[test]
fn test_task_agenda_accepts_modifier_style_date_filter_words() {
    let db = TestDb::new().note_with_content(
        "date-filter-words.org",
        &format!(
            r#":PROPERTIES:
:ID:       62626262-6262-4626-8626-626262626262
:END:
#+title: Date Filter Words
#+filetags: :agenda:

* TODO Tomorrow agenda task
SCHEDULED: <{tomorrow}>
* TODO Friday agenda task
SCHEDULED: <{friday}>
* TODO Later agenda task
SCHEDULED: <{later}>
"#,
            tomorrow = org_date(1),
            friday = upcoming_weekday_date(Weekday::Fri),
            later = org_date(8)
        ),
    );

    let (tomorrow_tasks, tomorrow_status) = db.run_json(&["task", "agenda", "date:tom"]);
    assert!(tomorrow_status.success());
    let tomorrow_titles = task_titles(&tomorrow_tasks);
    assert!(tomorrow_titles.contains(&"Tomorrow agenda task".to_string()));
    assert!(!tomorrow_titles.contains(&"Later agenda task".to_string()));

    let (friday_tasks, friday_status) = db.run_json(&["task", "agenda", "date:fri"]);
    assert!(friday_status.success());
    let friday_titles = task_titles(&friday_tasks);
    assert!(friday_titles.contains(&"Friday agenda task".to_string()));
    assert!(!friday_titles.contains(&"Later agenda task".to_string()));
}

#[test]
fn test_task_list_accepts_modifier_style_date_filter_words() {
    let db = TestDb::new().note_with_content(
        "list-date-filter-words.org",
        &format!(
            r#":PROPERTIES:
:ID:       63636363-6363-4636-8636-636363636363
:END:
#+title: List Date Filter Words

* TODO Friday list task
SCHEDULED: <{friday}>
* TODO Later list task
SCHEDULED: <{later}>
"#,
            friday = upcoming_weekday_date(Weekday::Fri),
            later = org_date(8)
        ),
    );

    let (friday_tasks, friday_status) = db.run_json(&["task", "list", "date:fri"]);
    assert!(friday_status.success());
    let friday_titles = task_titles(&friday_tasks);
    assert!(friday_titles.contains(&"Friday list task".to_string()));
    assert!(!friday_titles.contains(&"Later list task".to_string()));
}

#[test]
fn test_task_list_and_agenda_accept_modifier_style_after_before_filters() {
    let today = chrono::Local::now().date_naive();
    let tomorrow = today + chrono::Duration::days(1);
    let later = today + chrono::Duration::days(3);
    let before_noon = format!("before:{} 12:00", tomorrow.format("%Y-%m-%d"));
    let db = TestDb::new().note_with_content(
        "after-before-filter-words.org",
        &format!(
            r#":PROPERTIES:
:ID:       64646464-6464-4646-8646-646464646464
:END:
#+title: After Before Filter Words

* TODO Today timed task
SCHEDULED: <{today_late}>
* TODO Tomorrow morning task
SCHEDULED: <{tomorrow_morning}>
* TODO Tomorrow evening task
SCHEDULED: <{tomorrow_evening}>
* TODO Later task
SCHEDULED: <{later_date}>
"#,
            today_late = today.format("%Y-%m-%d %a 23:00"),
            tomorrow_morning = tomorrow.format("%Y-%m-%d %a 09:30"),
            tomorrow_evening = tomorrow.format("%Y-%m-%d %a 18:30"),
            later_date = later.format("%Y-%m-%d %a")
        ),
    );

    let (list_tasks, list_status) = db.run_json(&["task", "list", "after:tom", &before_noon]);
    assert!(list_status.success());
    assert_eq!(task_titles(&list_tasks), vec!["Tomorrow morning task"]);

    let (agenda_tasks, agenda_status) = db.run_json(&["task", "agenda", "after:tom", &before_noon]);
    assert!(agenda_status.success());
    assert_eq!(task_titles(&agenda_tasks), vec!["Tomorrow morning task"]);
}

#[test]
fn test_task_metadata_rejects_non_source_filters() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "projects",
        "state:TODO",
    ]);

    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("metadata commands only accept source filters")
    );
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_applies_client_side_filters() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![
        (
            "GET",
            "/tasks?limit=200",
            r#"{"results":[{"id":"keep","content":"Keep remote","project_id":"inbox","priority":4,"labels":["errand"],"due":{"date":"2026-06-01","string":"next week"}},{"id":"drop","content":"Drop remote","project_id":"work","priority":1,"labels":["work"],"due":{"date":"2026-06-01","string":"next week"}}],"next_cursor":null}"#,
        ),
        (
            "GET",
            "/projects?limit=200",
            r#"{"results":[{"id":"inbox","name":"Inbox"},{"id":"work","name":"Work"}],"next_cursor":null}"#,
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
            "tags:errand",
            "project:Inbox",
            "priority:A",
        ],
        &base_url,
    );
    handle.join().unwrap();

    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(task_titles(&v), vec!["Keep remote"]);
}

#[test]
fn test_task_list_metadata_text_omits_id_column() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "projects",
        "source:pkms",
    ]);
    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("Source"));
    assert!(stdout.contains("Name"));
    assert!(stdout.contains("Count"));
    assert!(
        !stdout.lines().next().unwrap_or_default().contains("Id"),
        "metadata table should not include Id column:\n{stdout}"
    );
}

fn task_titles(v: &serde_json::Value) -> Vec<String> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["title"].as_str().map(str::to_string))
        .collect()
}

fn task_default_sort_db() -> TestDb {
    TestDb::new().note_with_content(
        "sort-defaults.org",
        r#":PROPERTIES:
:ID:       55555555-5555-4555-8555-555555555555
:END:
#+title: Sort Defaults

* TODO [#A] Later urgent task
SCHEDULED: <2026-06-02 Tue>
* TODO [#C] Earlier low task
SCHEDULED: <2026-06-01 Mon>
* TODO [#B] Same day medium task
SCHEDULED: <2026-06-01 Mon>
"#,
    )
}

fn pkms_task_id_for_title(root: &std::path::Path, title: &str) -> String {
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    assert!(status.success());
    let source_id = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["title"] == title)
        .and_then(|item| item["source_id"].as_str())
        .unwrap_or_else(|| panic!("missing task title {title}: {v}"));
    format!("p{source_id}")
}

fn add_pkms_project_metadata_note(root: &std::path::Path) {
    std::fs::write(
        root.join("roam/common/20260524000000-task_metadata.org"),
        r#":PROPERTIES:
:ID:       77777777-7777-4777-8777-777777777777
:PROJECT: Note Project
:END:
#+title: Task Metadata
#+filetags: :filetag:

* TODO Note project task :headingtag:
* TODO Heading project task
:PROPERTIES:
:PROJECT: Heading Project
:END:
"#,
    )
    .unwrap();
}

fn assert_metadata_row(v: &serde_json::Value, source: &str, name: &str) {
    assert!(
        v["items"].as_array().unwrap().iter().any(|row| {
            row["source"].as_str() == Some(source) && row["name"].as_str() == Some(name)
        }),
        "missing metadata row {source}:{name}: {v}"
    );
}

#[test]
fn test_task_list_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "task",
        "list",
    ]);
    assert!(status.success(), "task list failed:\n{stdout}\n{stderr}");
    let first = stdout.lines().next().expect("expected at least one task");
    let v: serde_json::Value = serde_json::from_str(first).unwrap();
    assert_eq!(v["source"], "pkms");
    assert!(v.get("display_id").is_some());
    assert!(v.get("source_id").is_some());
    assert!(v.get("note_uuid").is_some());
    assert!(v.get("state").is_some());
    assert!(v.get("todo_state").is_none());
}

#[test]
fn test_task_list_source_pkms_uses_source_neutral_json() {
    let db = TestDb::new()
        .note(
            "focused.org",
            "Focused Tasks",
            "11111111-1111-4111-8111-111111111111",
        )
        .task("focused.org", "TODO", "Source neutral PKMS task");

    let (v, status) = db.run_json(&["task", "list", "source:pkms", "state:todo"]);

    assert!(status.success());
    assert_eq!(v["total"], 1);
    assert_eq!(v["items"][0]["source"], "pkms");
    assert_eq!(v["items"][0]["title"], "Source neutral PKMS task");
}

#[test]
fn test_task_agenda_focused_pkms_json_uses_source_neutral_item_shape() {
    let db = TestDb::new().note_with_content(
        "planned.org",
        r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: Focused Agenda

* TODO Focused planned task
SCHEDULED: <2026-05-29 Fri>
"#,
    );

    let (v, status) = db.run_json(&["task", "agenda"]);

    assert!(status.success());
    assert_eq!(v["total"], 1);
    assert_eq!(v["items"][0]["source"], "pkms");
    assert_eq!(v["items"][0]["title"], "Focused planned task");
    assert_eq!(v["items"][0]["note_title"], "Focused Agenda");
    assert_eq!(v["items"][0]["scheduled"]["date"], "2026-05-29");
    assert!(v["items"][0].get("heading_title").is_none());
}

#[test]
fn test_task_list_rejects_group_for_source_all() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Grouped Tasks",
            "33333333-3333-4333-8333-333333333333",
        )
        .task("tasks.org", "TODO", "Grouped task");

    let (stdout, _stderr, status) = db.run(&[
        "--output-format",
        "json",
        "task",
        "list",
        "--group",
        "state",
        "source:all",
    ]);

    assert!(!status.success());
    let v = assert_json_error_output(&["task", "list", "--group", "state", "source:all"], &stdout);
    assert_eq!(
        v["error"],
        "task list --group is available only for source:pkms"
    );
}

#[test]
fn test_task_list_filtered_view_preserves_canonical_pkms_ids() {
    let db = TestDb::new().note_with_content(
        "ids.org",
        r#":PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444444
:END:
#+title: Stable IDs

* TODO Earlier excluded task :excluded:
* TODO Later included task :included:
"#,
    );

    let (all, all_status) = db.run_json(&["task", "list"]);
    assert!(all_status.success());
    let all_items = all["items"].as_array().unwrap();
    let included_id = all_items
        .iter()
        .find(|item| item["title"] == "Later included task")
        .and_then(|item| item["source_id"].as_str())
        .map(str::to_string)
        .unwrap();

    let (filtered, filtered_status) = db.run_json(&["task", "list", "tag:included"]);

    assert!(filtered_status.success());
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["items"][0]["source_id"], included_id);
    assert_eq!(filtered["items"][0]["title"], "Later included task");
}

#[test]
fn test_task_list_canonical_ids_follow_timestamped_file_order() {
    let db = TestDb::new()
        .note_with_content(
            "common/20260524090000-zeta.org",
            r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: Zeta

* TODO Zeta first
* TODO Zeta second
"#,
        )
        .note_with_content(
            "common/20260524100000-latest.org",
            r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: Latest

* TODO Latest
"#,
        )
        .note_with_content(
            "common/20260524090000-alpha.org",
            r#":PROPERTIES:
:ID:       33333333-3333-4333-8333-333333333333
:END:
#+title: Alpha

* TODO Alpha
DEADLINE: <2024-01-01 Mon>
"#,
        )
        .note_with_content(
            "daily/2026-05-24.org",
            r#":PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444444
:END:
#+title: 2026-05-24

* TODO Daily
"#,
        )
        .note_with_content(
            "archive.org",
            r#":PROPERTIES:
:ID:       55555555-5555-4555-8555-555555555555
:END:
#+title: Archive

* TODO Archive
"#,
        );

    let (v, status) = db.run_json(&["task", "list"]);
    assert!(status.success());

    let source_id_for = |title: &str| {
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["title"] == title)
            .and_then(|item| item["source_id"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| panic!("missing task title {title}: {v}"))
    };

    assert_eq!(source_id_for("Latest"), "1");
    assert_eq!(source_id_for("Alpha"), "2");
    assert_eq!(source_id_for("Zeta first"), "3");
    assert_eq!(source_id_for("Zeta second"), "4");
    assert_eq!(source_id_for("Daily"), "5");
    assert_eq!(source_id_for("Archive"), "6");
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
            id,
            "show",
        ]);
        assert!(status.success(), "task show {id} failed");
        assert!(v.get("heading_title").is_some());
    }
}

#[test]
fn test_task_show_formats_inline_markup_when_terminal_formatting_is_forced() {
    let db = TestDb::new().note_with_content(
        "show-format.org",
        r#":PROPERTIES:
:ID:       67676767-6767-4767-8767-676767676767
:END:
#+title: Show Format

* TODO Review =literal= with ~orange~ for @alice
"#,
    );

    let config_home = setup_test_config_home();
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .args(["--db", db.root().to_str().unwrap(), "task", "p1", "show"])
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "forced task show failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("\x1b[2mliteral\x1b[0m"));
    assert!(stdout.contains("\x1b[38;5;166morange\x1b[0m"));
    assert!(stdout.contains("\x1b[38;5;39m@alice\x1b[0m"));
    assert!(!stdout.contains("=literal="));
    assert!(!stdout.contains("~orange~"));
}

#[test]
fn test_task_show_includes_parent_and_child_chain_ids() {
    let db = TestDb::new().note_with_content(
        "show-chain.org",
        r#":PROPERTIES:
:ID:       67676767-6767-4767-8767-676767676767
:END:
#+title: Show Chain

* TODO Parent task
** TODO Target task
*** TODO Child task
**** TODO Grandchild task
** TODO Sibling task
"#,
    );

    let (v, status) = db.run_json(&["task", "p2", "show"]);

    assert!(status.success());
    assert_eq!(v["heading_title"], "Target task");
    assert_eq!(v["parents"][0]["id"], 1);
    assert_eq!(v["parents"][0]["title"], "Parent task");
    assert_eq!(v["children"][0]["id"], 3);
    assert_eq!(v["children"][0]["title"], "Child task");
    assert_eq!(v["children"][1]["id"], 4);
    assert_eq!(v["children"][1]["title"], "Grandchild task");

    let (stdout, stderr, status) = db.run(&["task", "p2", "show"]);

    assert!(status.success(), "task show failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Parent chain (depends on):"));
    assert!(stdout.contains("p1 TODO Parent task"));
    assert!(stdout.contains("Child chain (blocks):"));
    assert!(stdout.contains("p3 TODO Child task"));
    assert!(stdout.contains("p4 TODO Grandchild task"));
}

#[test]
fn test_task_open_accepts_pkms_id_form() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "p1",
        "open",
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
        "todoist:123",
        "show",
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
        "p1",
        "state",
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
fn test_task_state_text_starts_with_task_title() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "p1",
        "state",
        "waiting",
    ]);
    assert!(status.success(), "task state failed:\n{stdout}\n{stderr}");
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("Task: Morning routine"));
    assert!(
        lines
            .next()
            .is_some_and(|line| line.contains("from TODO to WAITING")),
        "stdout: {stdout}"
    );
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
        "p1",
        "state",
        "waiting",
    ]);
    assert!(status.success());
    assert_eq!(v["new_state"], "WAITING");
    assert!(v.get("title").is_none());
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
        "pkms:1",
        "done",
    ]);
    assert!(status.success());
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "DONE");
}

#[test]
fn test_task_done_warns_when_task_ids_change() {
    let db = TestDb::new().note_with_content(
        "task-id-warning.org",
        r#":PROPERTIES:
:ID:       51515151-5151-4151-8151-515151515151
:END:
#+title: Task ID Warning

* TODO First task
* TODO Second task
"#,
    );

    let (stdout, stderr, status) = db.run(&["task", "p1", "done"]);

    assert!(status.success(), "task done failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("Changed"),
        "mutation output should stay on stdout:\n{stdout}"
    );
    assert!(
        stderr
            .lines()
            .last()
            .is_some_and(|line| line.contains("WARN: Task IDs changed")),
        "expected task ID warning on stderr:\n{stderr}"
    );
}

#[test]
fn test_task_state_does_not_warn_when_task_ids_stay_stable() {
    let db = TestDb::new().note_with_content(
        "stable-task-ids.org",
        r#":PROPERTIES:
:ID:       52525252-5252-4252-8252-525252525252
:END:
#+title: Stable Task IDs

* TODO First task
* TODO Second task
"#,
    );

    let (stdout, stderr, status) = db.run(&["task", "p1", "state", "waiting"]);

    assert!(status.success(), "task state failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("from TODO to WAITING"));
    assert!(
        !stderr.contains("Task IDs changed"),
        "state changes within the open group should not warn:\n{stderr}"
    );
}

#[test]
fn test_task_json_stdout_stays_parseable_when_task_ids_change() {
    let db = TestDb::new().note_with_content(
        "json-task-id-warning.org",
        r#":PROPERTIES:
:ID:       53535353-5353-4353-8353-535353535353
:END:
#+title: JSON Task ID Warning

* TODO First task
* TODO Second task
"#,
    );

    let (stdout, stderr, status) = db.run(&["--output-format", "json", "task", "p1", "done"]);

    assert!(status.success(), "task done failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "DONE");
    assert!(
        stderr.contains("WARN: Task IDs changed"),
        "expected warning on stderr:\n{stderr}"
    );
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
        "p1",
        "state",
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
        "todoist:123",
        "state",
        "DONE",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        v["error"],
        "Todoist support is not available in this build. Rebuild with --features todoist."
    );
}

#[test]
fn test_task_mod_pkms_accepts_state_modifier() {
    let (_dir, root) = setup_db();
    let task_id = pkms_task_id_for_title(&root, "High priority task");
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        &task_id,
        "mod",
        "state:waiting",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Status");
    assert_eq!(v["changes"][0]["old"], "TODO");
    assert_eq!(v["changes"][0]["new"], "WAITING");
    assert_eq!(v["item"]["state"], "WAITING");
    assert_eq!(v["item"]["status"], "open");
    let path = v["item"]["path"].as_str().unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("* WAITING [#A] High priority task"));
    assert!(!content.contains("* TODO [#A] High priority task"));
}

#[test]
fn test_task_mod_pkms_sets_and_clears_scheduled_date() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "sch:2026-07-01",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Scheduled");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-07-01");

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "sch:",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Scheduled");
    assert_eq!(v["item"]["scheduled"], serde_json::Value::Null);
}

#[test]
fn test_task_mod_planning_line_shift_does_not_warn_when_task_ids_stay_stable() {
    let db = TestDb::new().note_with_content(
        "planning-line-shift.org",
        r#":PROPERTIES:
:ID:       54545454-5454-4454-8454-545454545454
:END:
#+title: Planning Line Shift

* TODO First task
* TODO Second task
"#,
    );

    let (stdout, stderr, status) = db.run(&["task", "p1", "mod", "sch:2026-07-01"]);

    assert!(status.success(), "task mod failed:\n{stdout}\n{stderr}");
    let expected_change = if org_date(0) == "2026-07-01" {
        "Scheduled: None -> Scheduled: Today (2026-07-01)"
    } else if org_date(1) == "2026-07-01" {
        "Scheduled: None -> Scheduled: Tomorrow (2026-07-01)"
    } else {
        "Scheduled: None -> Scheduled: 2026-07-01"
    };
    assert!(stdout.contains(expected_change), "stdout:\n{stdout}");
    assert!(
        !stderr.contains("Task IDs changed"),
        "line shifts alone should not warn:\n{stderr}"
    );
}

#[test]
fn test_task_mod_pkms_sets_deadline_date() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "dl:2026-08-01",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Deadline");
    assert_eq!(v["item"]["deadline"]["date"], "2026-08-01");
}

#[test]
fn test_task_mod_pkms_accepts_add_style_metadata_modifiers() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "title:Updated task",
        "prio:b",
        "tag:phone,work",
        "proj:Focus",
        "desc:Follow up notes",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    let changed = v["changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|change| change["property"].as_str())
        .collect::<Vec<_>>();
    assert!(changed.contains(&"Title"));
    assert!(changed.contains(&"Priority"));
    assert!(changed.contains(&"Tags"));
    assert!(changed.contains(&"Project"));
    assert!(changed.contains(&"Description"));
    assert_eq!(v["item"]["title"], "Updated task");
    assert_eq!(v["item"]["priority"], "B");
    assert_eq!(v["item"]["project"], "Focus");
    assert!(
        v["item"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag == "phone")
    );
    assert!(
        v["item"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag == "work")
    );

    let path = v["item"]["path"].as_str().unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("* TODO [#B] Updated task :phone:work:"));
    assert!(content.contains(":PROJECT: Focus"));
    assert!(content.contains("Follow up notes"));
}

#[test]
fn test_task_mod_pkms_empty_values_clear_metadata() {
    let db = TestDb::new().note_with_content(
        "clear-modifiers.org",
        r#":PROPERTIES:
:ID:       43434343-4343-4343-8343-434343434343
:END:
#+title: Clear Modifiers

* TODO [#A] Clearable task :home:work:
SCHEDULED: <2026-05-24 Sun> DEADLINE: <2026-05-25 Mon>
:PROPERTIES:
:PROJECT: Focus
:END:

Body text.
* TODO Sibling
"#,
    );
    let path = db.root().join("roam/clear-modifiers.org");

    let (v, status) = db.run_json(&[
        "task", "p1", "mod", "tag:", "sch:", "dl:", "prio:", "project:", "desc:",
    ]);

    assert!(status.success());
    let changed = v["changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|change| change["property"].as_str())
        .collect::<Vec<_>>();
    assert!(changed.contains(&"Tags"));
    assert!(changed.contains(&"Scheduled"));
    assert!(changed.contains(&"Deadline"));
    assert!(changed.contains(&"Priority"));
    assert!(changed.contains(&"Project"));
    assert!(changed.contains(&"Description"));
    assert!(v["item"]["tags"].as_array().unwrap().is_empty());
    assert!(v["item"]["scheduled"].is_null());
    assert!(v["item"]["deadline"].is_null());
    assert!(v["item"]["priority"].is_null());
    assert!(v["item"]["project"].is_null());

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("* TODO Clearable task"));
    assert!(!content.contains("[#A]"));
    assert!(!content.contains(":home:work:"));
    assert!(!content.contains("SCHEDULED:"));
    assert!(!content.contains("DEADLINE:"));
    assert!(!content.contains(":PROJECT:"));
    assert!(!content.contains("Body text."));
}

#[test]
fn test_task_mod_pkms_dependency_moves_task_subtree() {
    let db = TestDb::new().note_with_content(
        "move-dependency.org",
        r#":PROPERTIES:
:ID:       42424242-4242-4242-8242-424242424242
:END:
#+title: Move Dependency

* Project
** TODO Target parent
Target body.
*** TODO Existing target child
** TODO Move source
Source body.
*** TODO Source child
Child body.
**** TODO Source grandchild
** TODO Later sibling
"#,
    );
    let path = db.root().join("roam/move-dependency.org");

    let (v, status) = db.run_json(&["task", "p3", "mod", "dep:1"]);

    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Dependency");
    assert_eq!(v["changes"][0]["new"], "p1");
    assert_eq!(v["item"]["title"], "Move source");
    assert_eq!(v["item"]["heading_level"], 3);

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content,
        r#":PROPERTIES:
:ID:       42424242-4242-4242-8242-424242424242
:END:
#+title: Move Dependency

* Project
** TODO Target parent
Target body.
*** TODO Existing target child
*** TODO Move source
Source body.
**** TODO Source child
Child body.
***** TODO Source grandchild
** TODO Later sibling
"#
    );
}

#[test]
fn test_task_mod_pkms_empty_dependency_removes_parent_dependency() {
    let db = TestDb::new().note_with_content(
        "clear-dependency.org",
        r#":PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444445
:END:
#+title: Clear Dependency

* Project
** TODO Parent task
Parent body.
*** TODO Child task
Child body.
**** TODO Grandchild task
** TODO Later sibling
"#,
    );
    let path = db.root().join("roam/clear-dependency.org");

    let (v, status) = db.run_json(&["task", "p2", "mod", "dep:"]);

    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Dependency");
    assert_eq!(v["changes"][0]["old"], "p1");
    assert!(v["changes"][0]["new"].is_null());
    assert_eq!(v["item"]["title"], "Child task");
    assert_eq!(v["item"]["heading_level"], 2);

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content,
        r#":PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444445
:END:
#+title: Clear Dependency

* Project
** TODO Parent task
Parent body.
** TODO Child task
Child body.
*** TODO Grandchild task
** TODO Later sibling
"#
    );
}

#[test]
fn test_task_mod_pkms_reports_text_property_diffs() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let tomorrow = org_date(1);
    let path = root.join("roam/common/20260602000000-mod-task.org");
    std::fs::write(
        &path,
        format!(
            r#":PROPERTIES:
:ID:       31313131-3131-4131-8131-313131313131
:END:
#+title: Mod Task

* TODO Mod task
SCHEDULED: <{today}>
"#
        ),
    )
    .unwrap();
    let (list, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    assert!(status.success());
    let id = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["title"].as_str() == Some("Mod task"))
        .and_then(|item| item["display_id"].as_str())
        .map(str::to_string)
        .unwrap();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        &id,
        "mod",
        "sch:tomorrow",
    ]);
    assert!(status.success(), "task mod failed:\n{stdout}\n{stderr}");
    assert_eq!(
        stdout.trim(),
        format!("Task: Mod task\nScheduled: Today ({today}) -> Scheduled: Tomorrow ({tomorrow})")
    );
}

#[test]
fn test_task_mod_pkms_no_changes_prints_message_and_exits_nonzero() {
    let (_dir, root) = setup_db();
    let task_id = pkms_task_id_for_title(&root, "High priority task");
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        &task_id,
        "mod",
        "sch:2026-05-10",
    ]);
    assert!(
        !status.success(),
        "task mod unexpectedly succeeded:\n{stdout}\n{stderr}"
    );
    assert_eq!(stdout.trim(), "Nothing changed");
}

#[test]
fn test_task_mod_rejects_implicit_title_text() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "Implicit",
        "title",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let error = v["error"].as_str().unwrap();
    assert!(error.contains("Unknown task modifier 'Implicit'"));
    assert!(error.contains("title:<text>"));
}

#[test]
fn test_task_mod_rejects_unknown_modifier() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "mod",
        "unknown:value",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Unknown task modifier 'unknown:value'")
    );
}

#[test]
fn test_task_schedule_and_deadline_id_subcommands_are_rejected() {
    let (_dir, root) = setup_db();
    for (subcommand, option) in [("schedule", "--due"), ("deadline", "--deadline")] {
        let (stdout, stderr, status) = run(&[
            "--db",
            root.to_str().unwrap(),
            "task",
            "p1",
            subcommand,
            option,
            "2026-06-01",
        ]);
        assert!(
            !status.success(),
            "task {subcommand} unexpectedly succeeded:\n{stdout}\n{stderr}"
        );
    }
}

#[test]
fn test_task_postpone_pkms_recurring_task_preserves_repeater() {
    let (_dir, root) = setup_db();
    let path = root.join("roam/common/20260525000002-recurring.org");
    std::fs::write(
        &path,
        r#":PROPERTIES:
:ID:       29292929-2929-4929-8929-292929292929
:END:
#+title: Recurring Tasks

* TODO Recurring call
SCHEDULED: <2026-05-24 Sun 09:30 +1w -1d>
"#,
    )
    .unwrap();
    let (list, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    assert!(status.success());
    let id = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["title"] == "Recurring call")
        .and_then(|item| item["display_id"].as_str())
        .map(str::to_string)
        .unwrap();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        &id,
        "postpone",
        "--to",
        "2026-06-01",
    ]);
    assert!(status.success());
    assert_eq!(v["action"], "postpone");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-06-01");
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("SCHEDULED: <2026-06-01 Mon 09:30 +1w -1d>"));
}

#[test]
fn test_task_postpone_pkms_non_recurring_task_fails() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "postpone",
        "--to",
        "2026-06-01",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v["error"].as_str().unwrap().contains("not recurring"));
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
    let item = items
        .iter()
        .find(|item| item["source_id"] == "abc")
        .expect("expected paginated todoist task");
    assert_eq!(item["source"], "todoist");
    assert_eq!(item["priority"], "A");
    assert_eq!(item["project"], "Inbox");
    assert_eq!(item["project_id"], "inbox");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_text_uses_bare_source_ids_sorted_by_remote_id() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks?limit=200",
        r#"{"results":[{"id":"200","content":"Second remote","priority":1,"labels":[]},{"id":"100","content":"First remote","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout
        .lines()
        .find(|line| line.contains("First remote"))
        .expect("expected first remote task");
    let second = stdout
        .lines()
        .find(|line| line.contains("Second remote"))
        .expect("expected second remote task");
    assert_eq!(first.split_whitespace().next().unwrap(), "100");
    assert_eq!(second.split_whitespace().next().unwrap(), "200");
    assert!(stdout.find("First remote") < stdout.find("Second remote"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_all_text_prefixes_each_source_id() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks?limit=200",
        r#"{"results":[{"id":"300","content":"Remote mixed","priority":1,"labels":[]}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &["--db", root.to_str().unwrap(), "task", "list", "source:all"],
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
    let pkms = stdout
        .lines()
        .find(|line| line.contains("High priority task"))
        .expect("expected pkms task");
    let todoist = stdout
        .lines()
        .find(|line| line.contains("Remote mixed"))
        .expect("expected todoist task");
    let pkms_id = pkms.split_whitespace().next().unwrap();
    assert!(
        pkms_id.starts_with('p'),
        "expected PKMS id prefix: {pkms_id}"
    );
    assert_eq!(todoist.split_whitespace().next().unwrap(), "todoist:300");
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
            "list",
            "projects",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"][0]["source"], "todoist");
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
            "list",
            "tags",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"][0]["source"], "todoist");
    assert_eq!(v["items"][0]["id"], "errand-id");
    assert_eq!(v["items"][0]["name"], "errand");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_projects_all_combines_pkms_and_todoist_metadata() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/projects?limit=200",
        r#"{"results":[{"id":"todoist-work","name":"Todoist Work"}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "projects",
            "source:all",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_metadata_row(&v, "pkms", "Note Project");
    assert_metadata_row(&v, "todoist", "Todoist Work");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_tags_all_combines_pkms_and_todoist_metadata() {
    let (_dir, root) = setup_db();
    add_pkms_project_metadata_note(&root);
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/labels?limit=200",
        r#"{"results":[{"id":"todoist-label","name":"todoist-tag"}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "tags",
            "source:all",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_metadata_row(&v, "pkms", "filetag");
    assert_metadata_row(&v, "todoist", "todoist-tag");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_text_splits_default_view_into_sections() {
    let (_dir, root) = setup_db();
    let overdue = org_date(-1);
    let today = org_date(0);
    let upcoming = org_date(1);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"old","content":"Overdue remote","priority":1,"labels":[],"due":{{"date":"{overdue}","string":"yesterday"}}}},{{"id":"today","content":"Today remote","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}},{{"id":"future","content":"Upcoming remote","priority":1,"labels":[],"due":{{"date":"{upcoming}","string":"tomorrow"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "agenda",
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let overdue_section = stdout.find(&section_box_start("Overdue")).expect("stdout");
    let today_section = stdout.find(&section_box_start("Today")).expect("stdout");
    let upcoming_section = stdout.find(&section_box_start("Upcoming")).expect("stdout");
    assert!(overdue_section < today_section);
    assert!(today_section < upcoming_section);
    assert!(overdue_section < stdout.find("Overdue remote").expect("stdout"));
    assert!(today_section < stdout.find("Today remote").expect("stdout"));
    assert!(upcoming_section < stdout.find("Upcoming remote").expect("stdout"));
    assert!(stdout.contains('╰'), "stdout:\n{stdout}");
    assert!(!stdout.contains("=== Overdue ==="), "stdout:\n{stdout}");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_text_gives_heading_available_width() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        r#"{"results":[{"id":"abc","content":"Alpha bravo charlie delta echo foxtrot golf hotel india","priority":1,"labels":[],"due":{"date":"2026-05-23","string":"today"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "agenda",
            "--columns=id,heading",
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Alpha bravo charlie delta echo foxtrot golf"),
        "heading wrapped too early:\n{stdout}"
    );
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
fn test_task_list_todoist_dates_use_pkms_display_format() {
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
            "task",
            "list",
            "source:todoist",
            "todoist.filter:today",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Id") && stdout.contains("Date") && stdout.contains("Type"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("Source"), "stdout: {stdout}");
    assert!(stdout.contains("─"), "stdout: {stdout}");
    assert!(stdout.contains("2026-05-23 Sat"), "stdout: {stdout}");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_datetime_dates_use_pkms_display_format() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=today&limit=200",
        r#"{"results":[{"id":"timed","content":"Timed task","priority":1,"labels":[],"due":{"date":"2026-05-25T07:00:00","string":"May 25 7am"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "task",
            "list",
            "source:todoist",
            "todoist.filter:today",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2026-05-25 Mon 07:00"), "stdout: {stdout}");
    assert!(!stdout.contains("2026-05-25T07:00:00"), "stdout: {stdout}");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_today_uses_date_alias_filter() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"today","content":"Today task","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "today",
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
fn test_task_agenda_todoist_dates_use_pkms_json_format() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"today","content":"Today task","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "today",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let raw = format!("<{today}>");
    assert_eq!(
        v["items"][0]["scheduled"]["raw"].as_str(),
        Some(raw.as_str())
    );
    assert_eq!(
        v["items"][0]["scheduled"]["date"].as_str(),
        Some(today.as_str())
    );
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_defaults_to_scheduled_filter() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        r#"{"results":[{"id":"scheduled","content":"Scheduled task","priority":1,"labels":[],"due":{"date":"2026-05-24","string":"tomorrow"}}],"next_cursor":null}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["source"], "todoist");
    assert_eq!(v["items"][0]["title"], "Scheduled task");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_agenda_todoist_overdue_uses_date_alias_filter() {
    let (_dir, root) = setup_db();
    let overdue = org_date(-1);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"old","content":"Overdue task","priority":1,"labels":[],"due":{{"date":"{overdue}","string":"yesterday"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "overdue",
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
fn test_task_agenda_todoist_week_uses_date_alias_filter() {
    let (_dir, root) = setup_db();
    let week = org_date(2);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"week","content":"Week task","priority":1,"labels":[],"due":{{"date":"{week}","string":"next week"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "week",
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
    let upcoming = org_date(1);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"future","content":"Future task","priority":1,"labels":[],"due":{{"date":"{upcoming}","string":"tomorrow"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "upcoming",
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
fn test_task_agenda_todoist_explicit_filter_overrides_fetch_query() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"p1","content":"Priority task","priority":4,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) =
        spawn_todoist_mock(vec![("GET", "/tasks/filter?query=p1&limit=200", body)]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "today",
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
fn test_task_today_todoist_uses_date_alias_filter() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"today","content":"Today shortcut","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
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
fn test_task_upcoming_todoist_days_filter_is_applied_locally() {
    let (_dir, root) = setup_db();
    let soon = org_date(1);
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"soon","content":"Soon shortcut","priority":1,"labels":[],"due":{{"date":"{soon}","string":"tomorrow"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
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
        "/tasks/filter?query=%21no%20date&limit=200",
        Box::leak(
            format!(
                r#"{{"results":[{{"id":"remote-today","content":"Remote shortcut today","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
            )
            .into_boxed_str(),
        ),
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
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

#[test]
fn test_task_inbox_pkms_requires_configured_inbox() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "inbox",
    ]);
    assert!(!status.success());
    assert!(
        stdout.contains("PKMS task inbox is not configured")
            || stderr.contains("PKMS task inbox is not configured")
    );
}

#[test]
fn test_task_inbox_pkms_uses_configured_note() {
    let (_dir, root) = setup_db();
    let inbox_path = root.join("roam/personal/20260525000000-inbox.org");
    std::fs::write(
        &inbox_path,
        r#":PROPERTIES:
:ID:       25252525-2525-4525-8525-252525252525
:END:
#+title: Inbox

* TODO Inbox task
* TODO [#A] Tagged inbox task :inbox:
"#,
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Inbox\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "inbox",
        ],
        &config,
    );
    assert!(status.success(), "task inbox failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let items = v["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["title"] == "Inbox task"));
    assert!(
        items
            .iter()
            .all(|item| item["path"] == inbox_path.display().to_string())
    );
}

#[test]
fn test_task_add_defaults_to_pkms_inbox() {
    let (_dir, root) = setup_db();
    let inbox_path = root.join("roam/personal/20260525000001-capture-inbox.org");
    std::fs::write(
        &inbox_path,
        r#":PROPERTIES:
:ID:       26262626-2626-4626-8626-262626262626
:END:
#+title: Capture Inbox
"#,
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Capture Inbox\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Capture new task",
            "due:2026-06-01",
            "deadline:2026-06-03",
            "priority:A",
            "label:inbox",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Capture new task");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-06-01");
    assert_eq!(v["item"]["deadline"]["date"], "2026-06-03");
    let content = std::fs::read_to_string(&inbox_path).unwrap();
    assert!(content.contains("* TODO [#A] Capture new task :inbox:"));
    assert!(content.contains("SCHEDULED: <2026-06-01"));
    assert!(content.contains("DEADLINE: <2026-06-03"));
}

#[test]
fn test_task_add_pkms_accepts_state_modifier() {
    let (_dir, root) = setup_db();
    let inbox_path = root.join("roam/personal/20260525000001-capture-inbox.org");
    std::fs::write(
        &inbox_path,
        r#":PROPERTIES:
:ID:       26262626-2626-4626-8626-262626262626
:END:
#+title: Capture Inbox
"#,
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Capture Inbox\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Waiting capture",
            "state:waiting",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Waiting capture");
    assert_eq!(v["item"]["state"], "WAITING");
    assert_eq!(v["item"]["status"], "open");
    let content = std::fs::read_to_string(&inbox_path).unwrap();
    assert!(content.contains("* WAITING Waiting capture"));
    assert!(!content.contains("* TODO Waiting capture"));
}

#[test]
fn test_task_add_pkms_accepts_scheduled_and_deadline_times() {
    let (_dir, root) = setup_db();
    let inbox_path = root.join("roam/personal/20260525000001-capture-inbox.org");
    std::fs::write(
        &inbox_path,
        r#":PROPERTIES:
:ID:       26262626-2626-4626-8626-262626262626
:END:
#+title: Capture Inbox
"#,
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Capture Inbox\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "prio:a",
            "sch:2025-05-26 09:30",
            "dl:2025-05-26 13:00",
            "Test",
            "with",
            "deadline",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Test with deadline");
    assert_eq!(v["item"]["scheduled"]["date"], "2025-05-26");
    assert_eq!(v["item"]["deadline"]["date"], "2025-05-26");
    let content = std::fs::read_to_string(&inbox_path).unwrap();
    assert!(content.contains("* TODO [#A] Test with deadline"));
    assert!(content.contains("SCHEDULED: <2025-05-26 Mon 09:30>"));
    assert!(content.contains("DEADLINE: <2025-05-26 Mon 13:00>"));
}

#[test]
fn test_task_add_pkms_accepts_modifiers_and_note_target() {
    let (_dir, root) = setup_db();
    let target_path = root.join("roam/personal/20260525000001-capture-target.org");
    std::fs::write(
        &target_path,
        r#":PROPERTIES:
:ID:       26262626-2626-4626-8626-262626262626
:END:
#+title: Capture Target
"#,
    )
    .unwrap();
    let today = org_date(0);
    let tomorrow = org_date(1);
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Modifier task",
            "sch:tod",
            "dead:tom",
            "prio:a",
            "tag:inbox,phone",
            "note:Capture Target",
            "desc:Body text",
        ],
        TEST_CONFIG,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Modifier task");
    assert_eq!(v["item"]["scheduled"]["date"], today);
    assert_eq!(v["item"]["deadline"]["date"], tomorrow);
    let content = std::fs::read_to_string(&target_path).unwrap();
    assert!(content.contains("* TODO [#A] Modifier task :inbox:phone:"));
    assert!(content.contains("SCHEDULED: <"));
    assert!(content.contains("DEADLINE: <"));
    assert!(content.contains("Body text"));
}

#[test]
fn test_task_add_pkms_dependency_appends_child_to_parent_subtree() {
    let db = TestDb::new().note_with_content(
        "dependencies.org",
        r#":PROPERTIES:
:ID:       41414141-4141-4141-8141-414141414141
:END:
#+title: Dependencies

* Project
** TODO Existing blocker
** TODO Parent task
Parent body.
*** TODO Existing child
Child body.
*** Notes
Notes inside the parent subtree.
* TODO Later sibling
"#,
    );
    let path = db.root().join("roam/dependencies.org");

    let (stdout, stderr, status) = db.run(&[
        "--output-format",
        "json",
        "task",
        "add",
        "Dependent task",
        "dep:2",
    ]);

    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Dependent task");
    assert_eq!(v["item"]["heading_level"], 3);

    let content = std::fs::read_to_string(&path).unwrap();
    let notes = content.find("Notes inside the parent subtree.").unwrap();
    let dependent = content.find("*** TODO Dependent task").unwrap();
    let sibling = content.find("* TODO Later sibling").unwrap();
    assert!(notes < dependent);
    assert!(dependent < sibling);
}

#[test]
fn test_task_add_pkms_accepts_weekday_and_word_prefix_dates() {
    let (_dir, root) = setup_db();
    let target_path = root.join("roam/personal/20260525000001-capture-target.org");
    std::fs::write(
        &target_path,
        r#":PROPERTIES:
:ID:       26262626-2626-4626-8626-262626262626
:END:
#+title: Capture Target
"#,
    )
    .unwrap();

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Weekday task",
            "sch:mon",
            "dead:to",
            "note:Capture Target",
        ],
        TEST_CONFIG,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        v["item"]["scheduled"]["date"],
        upcoming_weekday_date(Weekday::Mon)
    );
    assert_eq!(v["item"]["deadline"]["date"], org_date(0));

    let (stdout, _stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Ambiguous date",
            "sch:t",
            "note:Capture Target",
        ],
        TEST_CONFIG,
    );
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let error = v["error"].as_str().unwrap();
    assert!(error.contains("Ambiguous due date 't'"));
    assert!(error.contains("today"));
    assert!(error.contains("tuesday"));
    assert!(error.contains("thursday"));
}

#[test]
fn test_task_add_daily_inbox_appends_under_today_inbox_heading() {
    let (_dir, root) = setup_db();
    let today = chrono::Local::now().date_naive();
    let daily_path = root
        .join("roam/personal")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    std::fs::write(
        &daily_path,
        format!(
            r#":PROPERTIES:
:ID:       27272727-2727-4727-8727-272727272727
:END:
#+title: {}
#+filetags: :daily:

* Plan
Existing plan
* Inbox
** TODO Existing inbox task
* Notes
"#,
            today.format("%Y-%m-%d")
        ),
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"daily\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "Daily capture",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Daily capture");
    let content = std::fs::read_to_string(&daily_path).unwrap();
    let new_task = content.find("** TODO Daily capture").unwrap();
    let notes = content.find("* Notes").unwrap();
    assert!(new_task < notes);
}

#[test]
fn test_task_add_daily_inbox_uses_configured_daily_notes_dir() {
    let (_dir, root) = setup_db();
    let today = chrono::Local::now().date_naive();
    let daily_path = root
        .join("roam/dailies")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    let new_notes_daily_path = root
        .join("roam/new")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    let config = format!(
        "new_notes_dir = \"roam/new\"\ndaily_notes_dir = \"roam/dailies\"\n{TEST_CONFIG}\n[tasks]\ninbox = \"daily\"\n"
    );
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "Daily configured dir capture",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["source"], "pkms");
    assert_eq!(v["item"]["title"], "Daily configured dir capture");
    assert!(daily_path.exists());
    assert!(!new_notes_daily_path.exists());
    let content = std::fs::read_to_string(&daily_path).unwrap();
    assert!(content.contains(":PROPERTIES:\n:ID:"));
    assert!(content.contains(":END:\n#+title:"));
    let id = content
        .lines()
        .find_map(|line| line.strip_prefix(":ID:").map(str::trim))
        .expect("generated daily note should have an :ID:");
    uuid::Uuid::parse_str(id).expect("generated daily note should use a valid UUID");
    assert!(
        !content
            .lines()
            .any(|line| line.trim_start().starts_with("#+filetags:") && line.contains(":daily:")),
        "generated daily note should not get an automatic daily filetag:\n{content}"
    );
    assert!(content.contains("** TODO Daily configured dir capture"));
}

#[test]
fn test_task_add_daily_inbox_prefers_configured_daily_notes_dir_over_other_daily_file() {
    let (_dir, root) = setup_db();
    let today = chrono::Local::now().date_naive();
    let root_daily_path = root
        .join("roam")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    let configured_daily_path = root
        .join("roam/dailies")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    std::fs::create_dir_all(configured_daily_path.parent().unwrap()).unwrap();
    std::fs::write(
        &root_daily_path,
        format!(
            r#"#+title: {}
#+filetags: :daily:

* Inbox
** TODO Root inbox task
"#,
            today.format("%Y-%m-%d")
        ),
    )
    .unwrap();
    std::fs::write(
        &configured_daily_path,
        format!(
            r#"#+title: {}
#+filetags: :daily:

* Inbox
** TODO Configured inbox task
"#,
            today.format("%Y-%m-%d")
        ),
    )
    .unwrap();
    let config =
        format!("daily_notes_dir = \"roam/dailies\"\n{TEST_CONFIG}\n[tasks]\ninbox = \"daily\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "Daily configured preferred",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let root_content = std::fs::read_to_string(&root_daily_path).unwrap();
    let configured_content = std::fs::read_to_string(&configured_daily_path).unwrap();
    assert!(!root_content.contains("Daily configured preferred"));
    assert!(configured_content.contains("** TODO Daily configured preferred"));
}

#[test]
fn test_task_inbox_daily_lists_only_today_inbox_section() {
    let (_dir, root) = setup_db();
    let today = chrono::Local::now().date_naive();
    let daily_path = root
        .join("roam/personal")
        .join(format!("{}.org", today.format("%Y-%m-%d")));
    std::fs::write(
        &daily_path,
        format!(
            r#":PROPERTIES:
:ID:       28282828-2828-4828-8828-282828282828
:END:
#+title: {}
#+filetags: :daily:

* TODO Outside inbox
* Inbox
** TODO Inside inbox
* TODO After inbox
"#,
            today.format("%Y-%m-%d")
        ),
    )
    .unwrap();
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"daily\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "inbox",
        ],
        &config,
    );
    assert!(status.success(), "task inbox failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let titles: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert_eq!(titles, vec!["Inside inbox"]);
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_inbox_todoist_uses_inbox_filter() {
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
            "source:todoist",
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
    let (_dir, root) = setup_clean_db();
    let today = org_date(0);
    db_write(
        &root,
        "common/20260523000000-today-task.org",
        &format!(
            r#":PROPERTIES:
:ID:       abababab-abab-4aba-abab-abababababab
:END:
#+title: Today Task
#+filetags: :agenda:

* TODO Local today task
SCHEDULED: <{today}>
"#
        ),
    );
    let body = Box::leak(
        format!(
            r#"{{"results":[{{"id":"remote-today","content":"Remote today task","priority":1,"labels":[],"due":{{"date":"{today}","string":"today"}}}}],"next_cursor":null}}"#
        )
        .into_boxed_str(),
    );
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=%21no%20date&limit=200",
        body,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "agenda",
            "today",
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
            "todoist:abc",
            "show",
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
fn test_task_show_todoist_detects_pkms_note_marker() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/abc",
        r#"{"id":"abc","content":"Remote task","description":"User context\n\npkms:id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa","priority":2,"labels":["remote"]}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "show",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["note_uuid"], "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(v["note_title"], "Note A");
    assert!(v["body"].as_str().unwrap().contains("User context"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_list_todoist_detects_pkms_note_marker() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks?limit=200",
        r#"{"results":[{"id":"abc","content":"Remote task","description":"pkms:id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa","priority":2,"labels":["remote"]}],"next_cursor":null}"#,
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
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let item = &v["items"][0];
    assert_eq!(item["note_uuid"], "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(item["note_title"], "Note A");
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
            "source:todoist",
            "project:Inbox",
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
    assert_eq!(v["item"]["scheduled"]["raw"], "<2026-05-24>");
    assert_eq!(v["item"]["tags"], serde_json::json!(["errand"]));
    assert_eq!(v["item"]["project"], "Inbox");
    assert_eq!(v["item"]["project_id"], "inbox");
    assert_eq!(v["item"]["url"], "https://todoist.com/showTask?id=abc");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_note_modifier_is_rejected_before_api() {
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
            "source:todoist",
            "title:Call Alice",
            "description:Discuss launch",
            "note:Note A",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        v["error"].as_str().unwrap(),
        "note is available only for PKMS task creation."
    );
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
            "source:todoist",
            "title:Call Alice",
            "due:2026-05-24",
            "deadline:2026-05-30",
            "project:Inbox",
            "label:phone",
            "label:migration",
            "priority:B",
            "description:Discuss migration plan",
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
fn test_task_add_todoist_accepts_modifiers_and_date_shortcuts() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
    let tomorrow = org_date(1);
    let body = Box::leak(
        format!(
            r#"{{"id":"abc","content":"Call Alice","description":"Discuss migration plan","project_id":"inbox-id","priority":3,"labels":["phone","migration"],"due":{{"date":"{today}","string":"today"}},"deadline":{{"date":"{tomorrow}"}},"url":"https://todoist.com/showTask?id=abc"}}"#
        )
        .into_boxed_str(),
    );
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
                "due_date": today,
                "deadline_date": tomorrow
            }),
            body,
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
            "src:todoist",
            "title:Call Alice",
            "sch:tod",
            "dead:tom",
            "project:Inbox",
            "tag:phone,migration",
            "prio:b",
            "desc:Discuss migration plan",
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
    assert_eq!(v["item"]["source"], "todoist");
    assert_eq!(v["item"]["title"], "Call Alice");
    assert_eq!(v["item"]["priority"], "B");
    assert_eq!(v["item"]["scheduled"]["date"], today);
    assert_eq!(v["item"]["deadline"]["date"], tomorrow);
    assert_eq!(v["item"]["tags"], serde_json::json!(["phone", "migration"]));
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
            "source:todoist",
            "title:Call Alice",
            "due:2026-05-24",
            "project:Inbox",
            "priority:A",
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
            "source:todoist",
            "title:Call Alice",
            "project:Work",
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
            "source:todoist",
            "title:Call Alice",
            "priority:urgent",
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
            "source:todoist",
            "title:Call Alice",
            "due:next-week",
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
    let (base_url, handle) = spawn_todoist_mock(vec![("POST", "/tasks/abc/close", "")]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "done",
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
fn test_task_state_todoist_reopens_task() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![
        ("POST", "/tasks/abc/reopen", "null"),
        (
            "GET",
            "/tasks/abc",
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[]}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "state",
            "open",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["action"], "state-open");
    assert_eq!(v["item"]["display_id"], "todoist:abc");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_state_todoist_rejects_other_states() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "state",
            "waiting",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("open"));
    assert!(v["error"].as_str().unwrap().contains("done"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_postpone_todoist_updates_due_date() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[],"due":{"date":"2026-05-17","string":"every week","is_recurring":true}}"#,
        ),
        (
            "POST",
            "/tasks/abc",
            serde_json::json!({"due_date": "2026-05-24"}),
            r#"{}"#,
        ),
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[],"due":{"date":"2026-05-24","string":"2026-05-24"}}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "postpone",
            "--to",
            "2026-05-24",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["action"], "postpone");
    assert_eq!(v["item"]["display_id"], "todoist:abc");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-05-24");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_postpone_todoist_non_recurring_fails() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![(
        "GET",
        "/tasks/abc",
        serde_json::Value::Null,
        r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[],"due":{"date":"2026-05-17","string":"2026-05-17","is_recurring":false}}"#,
    )]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "postpone",
            "--to",
            "2026-05-24",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("not recurring"));
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_mod_todoist_can_clear_due_date() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[],"due":{"date":"2026-05-24","string":"2026-05-24"}}"#,
        ),
        (
            "POST",
            "/tasks/abc",
            serde_json::json!({"due_date": null}),
            r#"{}"#,
        ),
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[]}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "mod",
            "sch:",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Scheduled");
    assert!(v["item"]["scheduled"].is_null());
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_mod_todoist_sets_deadline_date() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[]}"#,
        ),
        (
            "POST",
            "/tasks/abc",
            serde_json::json!({"deadline_date": "2026-06-01"}),
            r#"{}"#,
        ),
        (
            "GET",
            "/tasks/abc",
            serde_json::Value::Null,
            r#"{"id":"abc","content":"Call Alice","description":"","priority":1,"labels":[],"deadline":{"date":"2026-06-01"}}"#,
        ),
    ]);
    let output = run_with_todoist_env(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "mod",
            "dl:2026-06-01",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Deadline");
    assert_eq!(v["item"]["deadline"]["date"], "2026-06-01");
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
            "todoist:abc",
            "done",
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
#[test]
fn test_todoist_http_logging_records_metadata_without_token() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock(vec![("POST", "/tasks/abc/close", "")]);
    let config_home = setup_test_config_home();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .args([
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "todoist:abc",
            "done",
        ])
        .env("TODOIST_API_TOKEN", "test-token")
        .env("PKMS_TODOIST_API_BASE_URL", &base_url)
        .env("PKMS_LOG_HTTP", "1")
        .env("COLUMNS", "120")
        .output()
        .unwrap();

    handle.join().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("todoist request"), "stderr: {stderr}");
    assert!(stderr.contains("todoist response"), "stderr: {stderr}");
    assert!(stderr.contains("method=\"POST\""), "stderr: {stderr}");
    assert!(
        stderr.contains("path=\"/tasks/abc/close\""),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("status=200"), "stderr: {stderr}");
    assert!(!stderr.contains("test-token"), "stderr: {stderr}");
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
        .env("COLUMNS", "120");
    output_with_timeout(&mut command)
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
        .env("PKMS_TODOIST_API_BASE_URL", base_url);
    output_with_timeout(&mut command)
}

#[cfg(feature = "todoist")]
fn output_with_timeout(command: &mut Command) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("failed to spawn pkms command");
    let deadline = Instant::now() + TODOIST_COMMAND_TIMEOUT;
    loop {
        if child
            .try_wait()
            .expect("failed to poll pkms command")
            .is_some()
        {
            return child
                .wait_with_output()
                .expect("failed to collect pkms command output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("failed to collect timed-out pkms command output");
            panic!(
                "pkms command timed out after {:?}\nstdout:\n{}\nstderr:\n{}",
                TODOIST_COMMAND_TIMEOUT,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
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
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, expected_body, body) in responses {
            let mut stream = accept_todoist_connection(&listener, method, expected_path);
            let request = read_todoist_request(&mut stream, method, expected_path);
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
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, body) in responses {
            let mut stream = accept_todoist_connection(&listener, method, expected_path);
            let request = read_todoist_request(&mut stream, method, expected_path);
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

#[cfg(feature = "todoist")]
fn accept_todoist_connection(
    listener: &TcpListener,
    method: &'static str,
    expected_path: &'static str,
) -> TcpStream {
    let deadline = Instant::now() + TODOIST_MOCK_ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    panic!(
                        "timed out after {:?} waiting for Todoist mock request {method} {expected_path}",
                        TODOIST_MOCK_ACCEPT_TIMEOUT
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                panic!("failed to accept Todoist mock request {method} {expected_path}: {err}")
            }
        }
    }
}

#[cfg(feature = "todoist")]
fn read_todoist_request(
    stream: &mut TcpStream,
    method: &'static str,
    expected_path: &'static str,
) -> String {
    stream
        .set_read_timeout(Some(TODOIST_MOCK_READ_TIMEOUT))
        .unwrap();
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).unwrap_or_else(|err| {
            panic!(
                "failed to read Todoist mock request {method} {expected_path} within {:?}: {err}",
                TODOIST_MOCK_READ_TIMEOUT
            )
        });
        if read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&buffer[..read]);
        if request_is_complete(&request_bytes) {
            break;
        }
    }
    String::from_utf8_lossy(&request_bytes).into_owned()
}

#[cfg(feature = "todoist")]
fn request_is_complete(request_bytes: &[u8]) -> bool {
    let request = String::from_utf8_lossy(request_bytes);
    let Some((headers, body)) = request.split_once("\r\n\r\n") else {
        return false;
    };
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    body.len() >= content_length
}
