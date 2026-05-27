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
