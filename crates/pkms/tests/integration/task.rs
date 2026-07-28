use super::*;
use chrono::{Datelike, Weekday};

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
}

#[test]
fn test_task_calendar_shows_current_month_with_weekdays() {
    let db = TestDb::clean();
    let today = chrono::Local::now().date_naive();
    let title = today.format("%B %Y").to_string();

    let (stdout, stderr, status) = db.run(&["task", "calendar"]);

    assert!(
        status.success(),
        "task calendar failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains(&title), "stdout:\n{stdout}");
    assert!(stdout.contains("Mo Tu We Th Fr Sa Su"), "stdout:\n{stdout}");
}

#[test]
fn test_task_calendar_marks_every_day_in_scheduled_and_deadline_ranges() {
    let today = chrono::Local::now().date_naive();
    let previous_month = today.with_day(1).unwrap().pred_opt().unwrap();
    let scheduled_start = previous_month.with_day(20).unwrap();
    let scheduled_end = previous_month.with_day(22).unwrap();
    let deadline_start = previous_month.with_day(24).unwrap();
    let deadline_end = previous_month.with_day(26).unwrap();
    let db = TestDb::new().note_with_content(
        "calendar-ranges.org",
        &format!(
            ":PROPERTIES:\n:ID:       12121212-1212-4212-8212-121212121212\n:END:\n#+title: Calendar ranges\n#+filetags: :agenda:\n\n* TODO Scheduled range\nSCHEDULED: <{scheduled_start}>--<{scheduled_end}>\n* TODO Deadline range\nDEADLINE: <{deadline_start}>--<{deadline_end}>\n"
        ),
    );

    let config_home = setup_test_config_home();
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .args([
            "--db",
            db.root().to_str().unwrap(),
            "task",
            "calendar",
            "-m",
            "-1",
        ])
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "task calendar failed:\n{stdout}\n{stderr}"
    );
    for day in 20..=22 {
        assert!(
            stdout.contains(&format!("\x1b[4m{day}\x1b[0m")),
            "scheduled day {day} was not underlined:\n{stdout}"
        );
    }
    for day in 24..=26 {
        assert!(
            stdout.contains(&format!("\x1b[31m{day}\x1b[0m")),
            "deadline day {day} was not colored:\n{stdout}"
        );
    }
}

#[test]
fn test_task_calendar_marks_unscheduled_task_on_daily_file_date() {
    let today = chrono::Local::now().date_naive();
    let daily_date = today
        .with_day(1)
        .unwrap()
        .pred_opt()
        .unwrap()
        .with_day(22)
        .unwrap();
    let db = TestDb::new().note_with_content(
        &format!("daily/{daily_date}.org"),
        ":PROPERTIES:\n:ID:       34343434-3434-4434-8434-343434343434\n:END:\n#+title: Daily tasks\n\n* TODO Unscheduled daily task\n",
    );

    let config_home = setup_test_config_home();
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .args([
            "--db",
            db.root().to_str().unwrap(),
            "task",
            "calendar",
            "-m",
            "-1",
        ])
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "task calendar failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("\x1b[4m22\x1b[0m"),
        "daily task date was not underlined:\n{stdout}"
    );
}

#[test]
fn test_task_list_help_shows_filters() {
    let (stdout, stderr, status) = run(&["task", "list", "--help"]);
    assert!(
        status.success(),
        "task list --help failed:\n{stdout}\n{stderr}"
    );
    assert!(stdout.contains("Filters:"));
    assert!(stdout.contains("source:pkms"));
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
    assert_eq!(v["items"][0]["display_id"], "1");
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
fn test_task_sort_and_group_validation_keeps_missing_db_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let missing_root = dir.path().join("definitely-missing-pkms-review");
    let missing = missing_root.to_str().unwrap();

    for command_args in [
        vec!["task", "list", "--sort", "unknown"],
        vec!["task", "list", "--group", "unknown"],
        vec!["task", "agenda", "--sort", "unknown"],
    ] {
        let mut args = vec!["--db", missing, "--output-format", "json"];
        args.extend(command_args);

        let (stdout, _stderr, status) = run(&args);

        assert!(!status.success(), "Expected failure for {args:?}");
        let value = assert_json_error_output(&args, &stdout);
        let error = value["error"].as_str().unwrap();
        assert!(
            error.contains("Failed to resolve db root"),
            "expected missing DB root to take precedence for {args:?}, got: {error}"
        );
        assert!(
            !error.contains("Unknown task"),
            "expected task validation not to take precedence for {args:?}, got: {error}"
        );
    }
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
    assert_eq!(v["items"][0]["display_id"], "1");
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
fn test_pkms_task_inherits_note_and_parent_heading_tags() {
    let db = TestDb::new().note_with_content(
        "inherited-tags.org",
        r#":PROPERTIES:
:ID:       91919191-9191-4191-8191-919191919191
:END:
#+title: Inherited Tags
#+filetags: :note:

* Area :area:shared:
** WAITING Parent task :taskparent:shared:
*** TODO Child task :child:shared:
"#,
    );

    let (list, status) = run_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
    ]);
    assert!(status.success());
    let child = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["title"] == "Child task")
        .unwrap();
    assert_eq!(
        child["tags"],
        serde_json::json!(["note", "area", "shared", "taskparent", "child"])
    );

    let child_id = child["source_id"].as_str().unwrap().to_string();
    let (show, status) = run_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        &child_id,
        "show",
    ]);
    assert!(status.success());
    assert_eq!(
        show["tags"],
        serde_json::json!(["note", "area", "shared", "taskparent", "child"])
    );
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
    source_id.to_string()
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
fn test_task_show_accepts_numeric_id() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "1",
        "show",
    ]);
    assert!(status.success(), "task show 1 failed");
    assert!(v.get("heading_title").is_some());
}

#[test]
fn test_task_show_includes_desc_body_from_added_final_task() {
    let db = TestDb::new().note("inbox.org", "Inbox", "11111111-1111-4111-8111-111111111111");
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Inbox\"\n");
    let root = db.root().to_str().unwrap();

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root,
            "task",
            "add",
            "some",
            "task",
            "desc:this is description",
        ],
        &config,
    );
    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");

    let (stdout, stderr, status) = run_with_config(&["--db", root, "task", "show", "1"], &config);
    assert!(status.success(), "task show failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains(
            "--- Content ---\n* TODO some task\n\nthis is description\n--- End Content ---"
        ),
        "stdout:\n{stdout}"
    );

    let args = ["--db", root, "--output-format", "json", "task", "show", "1"];
    let (stdout, stderr, status) = run_with_config(&args, &config);
    assert!(
        status.success(),
        "task show json failed:\n{stdout}\n{stderr}"
    );
    let v = assert_json_output(&args, &stdout);
    assert_eq!(v["content"], "* TODO some task\n\nthis is description");
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
        .args(["--db", db.root().to_str().unwrap(), "task", "1", "show"])
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

    let (v, status) = db.run_json(&["task", "2", "show"]);

    assert!(status.success());
    assert_eq!(v["heading_title"], "Target task");
    assert_eq!(v["parents"][0]["id"], 1);
    assert_eq!(v["parents"][0]["title"], "Parent task");
    assert_eq!(v["children"][0]["id"], 3);
    assert_eq!(v["children"][0]["title"], "Child task");
    assert_eq!(v["children"][1]["id"], 4);
    assert_eq!(v["children"][1]["title"], "Grandchild task");

    let (stdout, stderr, status) = db.run(&["task", "2", "show"]);

    assert!(status.success(), "task show failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Parent chain (depends on):"));
    assert!(stdout.contains("* 1 TODO Parent task"));
    assert!(stdout.contains("Child chain (blocks):"));
    assert!(stdout.contains("3 TODO Child task"));
    assert!(stdout.contains("4 TODO Grandchild task"));
}

#[test]
fn test_task_show_includes_non_task_parent_headings() {
    let db = TestDb::new().note_with_content(
        "show-heading-chain.org",
        r#":PROPERTIES:
:ID:       68686868-6868-4868-8868-686868686868
:END:
#+title: Show Heading Chain

* [[id:a1537bed-fc3b-4a4f-a065-07b690b9284b][Project]] heading
** TODO Parent task
*** Section heading
**** TODO Target task
"#,
    );

    let (v, status) = db.run_json(&["task", "2", "show"]);

    assert!(status.success());
    assert_eq!(v["parents"][0]["id"], serde_json::Value::Null);
    assert_eq!(v["parents"][0]["title"], "Project heading");
    assert_eq!(v["parents"][0]["todo_state"], serde_json::Value::Null);
    assert_eq!(v["parents"][1]["id"], 1);
    assert_eq!(v["parents"][1]["title"], "Parent task");
    assert_eq!(v["parents"][2]["id"], serde_json::Value::Null);
    assert_eq!(v["parents"][2]["title"], "Section heading");

    let (stdout, stderr, status) = db.run(&["task", "2", "show"]);

    assert!(status.success(), "task show failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("* Project heading (line 6)"));
    assert!(!stdout.contains("[[id:"));
    assert!(stdout.contains("** 1 TODO Parent task (line 7)"));
    assert!(stdout.contains("*** Section heading (line 8)"));
}

#[test]
fn test_task_open_accepts_pkms_id_form() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "1",
        "open",
        "--editor",
        "true",
    ]);
    assert!(status.success(), "task open failed:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Opening:"));
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
        "1",
        "state",
        "waiting",
        "--dry-run",
    ]);
    assert!(status.success());
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "WAITING");
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["new_id"], serde_json::Value::Null);
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
        "1",
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
        "1",
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
        "1",
        "done",
    ]);
    assert!(status.success());
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "DONE");
}

#[test]
fn test_task_done_rejects_parent_with_open_child() {
    let db = TestDb::new().note_with_content(
        "state-children.org",
        r#":PROPERTIES:
:ID:       61616161-6161-4161-8161-616161616161
:END:
#+title: State Children

* TODO Parent task
** TODO Child task A
** DONE Child task B
"#,
    );
    let task_id = pkms_task_id_for_title(db.root(), "Parent task");

    let (v, status) = db.run_json(&["task", &task_id, "done"]);

    assert!(!status.success());
    assert_eq!(
        v["error"],
        "Task state from TODO to DONE blocked by \"TODO Child task A\"."
    );
    let content = std::fs::read_to_string(db.root().join("roam/state-children.org")).unwrap();
    assert!(content.contains("* TODO Parent task"));
    assert!(!content.contains("* DONE Parent task"));
}

#[test]
fn test_task_state_allows_reopening_closed_parent_with_open_child() {
    let db = TestDb::new().note_with_content(
        "state-reopen.org",
        r#":PROPERTIES:
:ID:       62626262-6262-4262-8262-626262626262
:END:
#+title: State Reopen

* DONE Parent task
** TODO Child task A
"#,
    );
    let task_id = pkms_task_id_for_title(db.root(), "Parent task");

    let (v, status) = db.run_json(&["task", &task_id, "state", "TODO"]);

    assert!(status.success());
    assert_eq!(v["old_state"], "DONE");
    assert_eq!(v["new_state"], "TODO");
    let content = std::fs::read_to_string(db.root().join("roam/state-reopen.org")).unwrap();
    assert!(content.contains("* TODO Parent task"));
    assert!(content.contains("** TODO Child task A"));
}

#[test]
fn test_task_mod_state_rejects_parent_with_open_child() {
    let db = TestDb::new().note_with_content(
        "mod-state-children.org",
        r#":PROPERTIES:
:ID:       63636363-6363-4363-8363-636363636363
:END:
#+title: Mod State Children

* TODO Parent task
** WAITING Child task A
"#,
    );
    let task_id = pkms_task_id_for_title(db.root(), "Parent task");

    let (v, status) = db.run_json(&["task", &task_id, "mod", "state:DONE"]);

    assert!(!status.success());
    assert_eq!(
        v["error"],
        "Task state from TODO to DONE blocked by \"WAITING Child task A\"."
    );
    let content = std::fs::read_to_string(db.root().join("roam/mod-state-children.org")).unwrap();
    assert!(content.contains("* TODO Parent task"));
    assert!(!content.contains("* DONE Parent task"));
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

    let (stdout, stderr, status) = db.run(&["task", "1", "done"]);

    assert!(status.success(), "task done failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("Changed"),
        "mutation output should stay on stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("New task ID: 2"),
        "expected the completed task's new canonical ID:\n{stdout}"
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

    let (stdout, stderr, status) = db.run(&["task", "1", "state", "waiting"]);

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

    let (stdout, stderr, status) = db.run(&["--output-format", "json", "task", "1", "done"]);

    assert!(status.success(), "task done failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["old_state"], "TODO");
    assert_eq!(v["new_state"], "DONE");
    assert_eq!(v["new_id"], "2");
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
        "1",
        "state",
        "UNKNOWN",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v["error"].as_str().unwrap().contains("Valid states:"));
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
        "1",
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
        "1",
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

    let (stdout, stderr, status) = db.run(&["task", "1", "mod", "sch:2026-07-01"]);

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
        "1",
        "mod",
        "dl:2026-08-01",
    ]);
    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Deadline");
    assert_eq!(v["item"]["deadline"]["date"], "2026-08-01");
}

#[test]
fn test_task_mod_pkms_assumes_today_for_time_without_date() {
    let db = TestDb::new()
        .note(
            "timed-task.org",
            "Timed Task",
            "27272727-2727-4727-8727-272727272727",
        )
        .task("timed-task.org", "TODO", "Task to modify");
    let today = org_date(0);
    let (v, status) = db.run_json(&["task", "1", "mod", "sch:09:30"]);

    assert!(status.success());
    assert_eq!(v["item"]["scheduled"]["date"], today);
    assert!(
        v["item"]["scheduled"]["raw"]
            .as_str()
            .unwrap()
            .contains("09:30")
    );
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
        "1",
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
fn test_task_mod_pkms_removes_redundant_project_matching_note_project() {
    let db = TestDb::new().note_with_content(
        "redundant-project.org",
        r#":PROPERTIES:
:ID:       70707070-7070-4070-8070-707070707070
:PROJECT: Focus
:END:
#+title: Redundant Project

* TODO Project task
:PROPERTIES:
:PROJECT: Work
:END:
"#,
    );
    let path = db.root().join("roam/redundant-project.org");

    let (v, status) = db.run_json(&["task", "1", "mod", "project:focus"]);

    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Project");
    assert_eq!(v["changes"][0]["old"], "Work");
    assert_eq!(v["changes"][0]["new"], serde_json::Value::Null);
    assert_eq!(v["item"]["project"], "Focus");
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content.matches(":PROJECT: Focus").count(), 1);
    assert!(!content.contains(":PROJECT: Work"));
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
        "task", "1", "mod", "tag:", "sch:", "dl:", "prio:", "project:", "desc:",
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

    let (v, status) = db.run_json(&["task", "3", "mod", "dep:1"]);

    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Dependency");
    assert_eq!(v["changes"][0]["new"], "1");
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

    let (v, status) = db.run_json(&["task", "2", "mod", "dep:"]);

    assert!(status.success());
    assert_eq!(v["changed"], true);
    assert_eq!(v["changes"][0]["property"], "Dependency");
    assert_eq!(v["changes"][0]["old"], "1");
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
        "1",
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
        "1",
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
fn test_task_postpone_pkms_defaults_to_next_occurrence() {
    let (_dir, root) = setup_db();
    let path = root.join("roam/common/20260525000002-recurring-default.org");
    std::fs::write(
        &path,
        r#":PROPERTIES:
:ID:       30303030-3030-4030-8030-303030303030
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
    ]);

    assert!(status.success());
    assert_eq!(v["action"], "postpone");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-05-31");
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("SCHEDULED: <2026-05-31 Sun 09:30 +1w -1d>"));
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
        "1",
        "postpone",
        "--to",
        "2026-06-01",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v["error"].as_str().unwrap().contains("not recurring"));
}

#[test]
fn test_task_postpone_rejects_date_alias() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "1",
        "postpone",
        "--date",
        "2026-06-01",
    ]);

    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("Unexpected argument for task postpone: --date")
    );
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
fn test_task_add_pkms_assumes_today_for_times_without_dates() {
    let db = TestDb::new().note("inbox.org", "Inbox", "26262626-2626-4626-8626-262626262626");
    let today = org_date(0);
    let config = format!("{TEST_CONFIG}\n[tasks]\ninbox = \"Inbox\"\n");
    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            db.root().to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "add",
            "title:Timed task",
            "sch:09:30",
            "dl:13:00",
        ],
        &config,
    );

    assert!(status.success(), "task add failed:\n{stdout}\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["item"]["scheduled"]["date"], today);
    assert_eq!(v["item"]["deadline"]["date"], today);
    assert!(
        v["item"]["scheduled"]["raw"]
            .as_str()
            .unwrap()
            .contains("09:30")
    );
    assert!(
        v["item"]["deadline"]["raw"]
            .as_str()
            .unwrap()
            .contains("13:00")
    );
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
fn test_task_add_pkms_omits_project_matching_note_project() {
    let db = TestDb::new().note_with_content(
        "matching-project.org",
        r#":PROPERTIES:
:ID:       68686868-6868-4868-8868-686868686868
:PROJECT: Focus
:END:
#+title: Matching Project
"#,
    );
    let path = db.root().join("roam/matching-project.org");

    let (v, status) = db.run_json(&[
        "task",
        "add",
        "title:Inherited project task",
        "project:focus",
        "note:Matching Project",
    ]);

    assert!(status.success());
    assert_eq!(v["item"]["project"], "Focus");
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content.matches(":PROJECT: Focus").count(), 1);
}

#[test]
fn test_task_add_pkms_writes_project_when_note_has_no_project() {
    let db = TestDb::new().note(
        "projectless-note.org",
        "Projectless Note",
        "71717171-7171-4171-8171-717171717171",
    );
    let path = db.root().join("roam/projectless-note.org");

    let (v, status) = db.run_json(&[
        "task",
        "add",
        "title:Assigned project task",
        "project:Focus",
        "note:Projectless Note",
    ]);

    assert!(status.success());
    assert_eq!(v["item"]["project"], "Focus");
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("* TODO Assigned project task\n:PROPERTIES:\n:PROJECT: Focus\n:END:"));
}

#[test]
fn test_task_add_pkms_writes_project_differing_from_note_project() {
    let db = TestDb::new().note_with_content(
        "different-project.org",
        r#":PROPERTIES:
:ID:       69696969-6969-4969-8969-696969696969
:PROJECT: Work
:END:
#+title: Different Project
"#,
    );
    let path = db.root().join("roam/different-project.org");

    let (v, status) = db.run_json(&[
        "task",
        "add",
        "title:Specific project task",
        "project:Focus",
        "note:Different Project",
    ]);

    assert!(status.success());
    assert_eq!(v["item"]["project"], "Focus");
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("* TODO Specific project task\n:PROPERTIES:\n:PROJECT: Focus\n:END:"));
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
