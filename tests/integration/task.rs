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
    assert!(stdout.contains("inbox"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("open"));
    assert!(stdout.contains("state"));
    assert!(stdout.contains("done"));
    assert!(stdout.contains("add"));
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
fn test_task_list_matches_todo_json() {
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
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_matches_todo_text() {
    let (_dir, root) = setup_db();
    let (task_stdout, task_stderr, task_status) =
        run(&["--db", root.to_str().unwrap(), "task", "list"]);
    let (todo_stdout, todo_stderr, todo_status) = run(&["--db", root.to_str().unwrap(), "todo"]);

    assert!(
        task_status.success(),
        "task list failed:\n{task_stdout}\n{task_stderr}"
    );
    assert!(
        todo_status.success(),
        "todo failed:\n{todo_stdout}\n{todo_stderr}"
    );
    assert_eq!(task_stdout, todo_stdout);
}

#[test]
fn test_task_list_limit_json_matches_todo() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--limit",
        "1",
    ]);
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--limit",
        "1",
    ]);

    assert!(task_status.success());
    assert!(todo_status.success());
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_group_state_matches_todo_json() {
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
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--group",
        "state",
    ]);
    assert!(task_status.success());
    assert!(todo_status.success());
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_from_stdin_matches_todo_scope() {
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
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--scope",
        "Agenda Item",
    ]);
    assert!(todo_status.success());
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_legacy_schema_matches_todo_for_filters() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--output-schema",
        "legacy",
        "state:TODO",
        "tags:agenda",
        "prio:A",
    ]);
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--prio",
        "A",
    ]);
    assert!(task_status.success());
    assert!(todo_status.success());
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_compatibility_filter_flags_match_todo() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "list",
        "--output-schema",
        "legacy",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--type",
        "SCHED",
        "--prio",
        "A",
    ]);
    let (todo, todo_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--type",
        "SCHED",
        "--prio",
        "A",
    ]);
    assert!(task_status.success());
    assert!(todo_status.success());
    assert_eq!(task, todo);
}

#[test]
fn test_task_list_pkms_text_uses_bare_source_ids() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "task",
        "list",
        "--limit",
        "1",
    ]);

    assert!(status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    let row = stdout
        .lines()
        .find(|line| line.contains("High priority task"))
        .expect("expected a task row");
    let id = row.split_whitespace().next().unwrap();
    assert_eq!(id, "1");
}

#[test]
fn test_task_agenda_matches_agenda_json() {
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
    assert_eq!(task, agenda);
}

#[test]
fn test_task_agenda_matches_agenda_text() {
    let (_dir, root) = setup_db();
    let (task_stdout, task_stderr, task_status) =
        run(&["--db", root.to_str().unwrap(), "task", "agenda"]);
    let (agenda_stdout, agenda_stderr, agenda_status) =
        run(&["--db", root.to_str().unwrap(), "agenda"]);

    assert!(
        task_status.success(),
        "task agenda failed:\n{task_stdout}\n{task_stderr}"
    );
    assert!(
        agenda_status.success(),
        "agenda failed:\n{agenda_stdout}\n{agenda_stderr}"
    );
    assert_eq!(task_stdout, agenda_stdout);
}

#[test]
fn test_task_agenda_legacy_schema_matches_agenda_for_filters() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--output-schema",
        "legacy",
        "state:TODO",
        "tags:agenda",
        "type:SCHED",
        "prio:A",
    ]);
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--type",
        "SCHED",
        "--prio",
        "A",
    ]);
    assert!(task_status.success());
    assert!(agenda_status.success());
    assert_eq!(task, agenda);
}

#[test]
fn test_task_agenda_date_upcoming_legacy_schema_matches_agenda_upcoming() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--output-schema",
        "legacy",
        "date:upcoming",
    ]);
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--upcoming",
    ]);
    assert!(task_status.success());
    assert!(agenda_status.success());
    assert_eq!(task, agenda);
}

#[test]
fn test_task_agenda_compatibility_filter_flags_match_agenda() {
    let (_dir, root) = setup_db();
    let (task, task_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "agenda",
        "--output-schema",
        "legacy",
        "--today",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--type",
        "SCHED",
    ]);
    let (agenda, agenda_status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--today",
        "--state",
        "TODO",
        "--tags",
        "agenda",
        "--type",
        "SCHED",
    ]);
    assert!(task_status.success());
    assert!(agenda_status.success());
    assert_eq!(task, agenda);
}

#[test]
fn test_legacy_task_commands_warn_on_stderr_only() {
    let (_dir, root) = setup_db();
    let (todo_stdout, todo_stderr, todo_status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--limit",
        "1",
    ]);
    assert!(todo_status.success());
    assert!(todo_stdout.trim_start().starts_with('{'));
    assert!(todo_stderr.contains("`pkms todo` is deprecated"));

    let (agenda_stdout, agenda_stderr, agenda_status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--limit",
        "1",
    ]);
    assert!(agenda_status.success());
    assert!(agenda_stdout.trim_start().starts_with('{'));
    assert!(agenda_stderr.contains("`pkms agenda` is deprecated"));
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
fn test_task_list_source_neutral_sort_supports_legacy_file_field() {
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
fn test_task_agenda_source_neutral_sort_supports_legacy_fields() {
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
fn test_task_agenda_accepts_date_filters() {
    let (_dir, root) = setup_db();
    let today = org_date(0);
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
    assert!(upcoming_titles.contains(&"Parent task".to_string()));
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
    assert!(task_titles(&week_tasks).contains(&"Parent task".to_string()));
}

#[test]
fn test_task_agenda_accepts_exact_and_bare_date_filters() {
    let (_dir, root) = setup_db();
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
    assert!(task_titles(&upcoming_tasks).contains(&"Parent task".to_string()));
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
    let (task_stdout, task_stderr, task_status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "task",
        "list",
    ]);
    let (todo_stdout, todo_stderr, todo_status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "todo",
    ]);
    assert!(
        task_status.success(),
        "task list failed:\n{task_stdout}\n{task_stderr}"
    );
    assert!(
        todo_status.success(),
        "todo failed:\n{todo_stdout}\n{todo_stderr}"
    );
    assert_eq!(task_stdout, todo_stdout);
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
fn test_task_schedule_pkms_sets_and_clears_scheduled_date() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "schedule",
        "--due",
        "2026-07-01",
    ]);
    assert!(status.success());
    assert_eq!(v["action"], "schedule");
    assert_eq!(v["item"]["scheduled"]["date"], "2026-07-01");

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "schedule",
        "--due",
        "none",
    ]);
    assert!(status.success());
    assert_eq!(v["action"], "unschedule");
    assert_eq!(v["item"]["scheduled"], serde_json::Value::Null);
}

#[test]
fn test_task_deadline_pkms_sets_deadline_date() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "task",
        "p1",
        "deadline",
        "--deadline",
        "2026-08-01",
    ]);
    assert!(status.success());
    assert_eq!(v["action"], "deadline");
    assert_eq!(v["item"]["deadline"]["date"], "2026-08-01");
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
        .find(|item| item["heading_title"] == "Recurring call")
        .and_then(|item| item["id"].as_u64())
        .unwrap()
        .to_string();
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
    assert_eq!(items[0]["source"], "todoist");
    assert_eq!(items[0]["source_id"], "abc");
    assert_eq!(items[0]["priority"], "A");
    assert_eq!(items[0]["project"], "Inbox");
    assert_eq!(items[0]["project_id"], "inbox");
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
    assert_eq!(pkms.split_whitespace().next().unwrap(), "p1");
    assert_eq!(todoist.split_whitespace().next().unwrap(), "t300");
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
            "today",
            "source:todoist",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["items"][0]["scheduled"]["raw"], "<2026-05-23>");
    assert_eq!(v["items"][0]["scheduled"]["date"], "2026-05-23");
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
    let (base_url, handle) = spawn_todoist_mock(vec![(
        "GET",
        "/tasks/filter?query=due%20after%3A%20today%20%26%20next%207%20days&limit=200",
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
            "--title",
            "Capture new task",
            "--due",
            "2026-06-01",
            "--deadline",
            "2026-06-03",
            "--priority",
            "A",
            "--label",
            "inbox",
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
    assert_eq!(v["item"]["scheduled"]["raw"], "<2026-05-24>");
    assert_eq!(v["item"]["tags"], serde_json::json!(["errand"]));
    assert_eq!(v["item"]["project"], "Inbox");
    assert_eq!(v["item"]["project_id"], "inbox");
    assert_eq!(v["item"]["url"], "https://todoist.com/showTask?id=abc");
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_add_todoist_note_marker_preserves_description() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
        (
            "GET",
            "/projects?limit=200",
            serde_json::Value::Null,
            r#"{"results":[],"next_cursor":null}"#,
        ),
        (
            "POST",
            "/tasks",
            serde_json::json!({
                "content": "Call Alice",
                "description": "Discuss launch\n\npkms:id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
            }),
            r#"{"id":"abc","content":"Call Alice","description":"Discuss launch\n\npkms:id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa","priority":1,"labels":[]}"#,
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
            "--description",
            "Discuss launch",
            "--note",
            "Note A",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        v["item"]["note_uuid"],
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
    );
    assert_eq!(v["item"]["note_title"], "Note A");
    assert!(
        v["item"]["body"]
            .as_str()
            .unwrap()
            .contains("Discuss launch")
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
fn test_task_schedule_todoist_can_clear_due_date() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
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
            "schedule",
            "--due",
            "none",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["action"], "unschedule");
    assert!(v["item"]["scheduled"].is_null());
}

#[cfg(feature = "todoist")]
#[test]
fn test_task_deadline_todoist_sets_deadline_date() {
    let (_dir, root) = setup_db();
    let (base_url, handle) = spawn_todoist_mock_expect_bodies(vec![
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
            "deadline",
            "--deadline",
            "2026-06-01",
        ],
        &base_url,
    );
    handle.join().unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["changed"], true);
    assert_eq!(v["action"], "deadline");
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
fn run_with_todoist_env(args: &[&str], base_url: &str) -> std::process::Output {
    let config_home = setup_test_config_home();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    command
        .args(args)
        .env("TODOIST_API_TOKEN", "test-token")
        .env("PKMS_TODOIST_API_BASE_URL", base_url)
        .env("COLUMNS", "120")
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
