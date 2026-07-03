use super::*;

#[test]
fn test_db_runs_commands_against_its_root() {
    let db = TestDb::fixture();
    let (value, status) = db.run_json(&["info"]);

    assert!(status.success());
    assert_eq!(
        value["config"]["db_root"].as_str(),
        Some(db.root().to_str().unwrap())
    );
}

#[test]
fn test_db_writes_focused_roam_fixtures() {
    let db = TestDb::clean();
    db.write_roam(
        "focused.org",
        r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: Focused Fixture
"#,
    );

    let (value, status) = db.run_json(&["resolve", "--title", "Focused Fixture"]);

    assert!(status.success());
    assert_eq!(value["results"][0]["title"], "Focused Fixture");
}

#[test]
fn test_db_builder_creates_note_and_task_fixtures() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Task Fixtures",
            "11111111-1111-4111-8111-111111111111",
        )
        .task("tasks.org", "TODO", "Exercise builder");

    let (value, status) = db.run_json(&["task", "list"]);

    assert!(status.success());
    assert_eq!(value["items"][0]["note_title"], "Task Fixtures");
    assert_eq!(value["items"][0]["title"], "Exercise builder");
}
