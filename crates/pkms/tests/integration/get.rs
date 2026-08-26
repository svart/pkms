use super::*;

#[test]
fn test_get_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "get", "Note A", "--links"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note B"));
}

#[test]
fn test_get_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "get",
        "Note A",
        "--links",
    ]);
    assert!(status.success());
    assert_eq!(v["node"]["title"], "Note A");
    assert!(v.get("neighbors").is_some());
}

#[test]
fn test_get_no_content() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "get",
        "Note A",
        "--no-content",
    ]);
    assert!(status.success());
    assert!(!stdout.contains("--- Content ---"));
}

#[test]
fn test_get_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "get", "Nonexistent"]);
    assert!(!status.success());
}

#[test]
fn test_get_headings_with_uuids() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-heading-uuid-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       55555555-5555-4555-8555-555555555555
:END:
#+title: Get Heading UUIDs Test

* Section One
:PROPERTIES:
:ID:       66666666-6666-4666-8666-666666666666
:END:
Text
* Section Two
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "get",
        "Get Heading UUIDs Test",
        "--headings",
    ]);
    assert!(status.success());
    assert!(v["node"]["headings"].is_array());
    let headings = v["node"]["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2);
    assert_eq!(
        headings[0]["uuid"], "66666666-6666-4666-8666-666666666666",
        "first heading should have uuid"
    );
    assert!(
        headings[1].get("uuid").is_none(),
        "second heading should not have uuid"
    );
}

#[test]
fn test_get_headings_uses_configured_todo_states() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "configured-get-states.org",
        r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-8ddd-dddddddddddd
:END:
#+title: Configured Get States

* PKMS operational logging
* NEXT Configured task
"#,
    );
    let config = r#"[agenda]
open_todo_states = ["NEXT"]
closed_todo_states = ["DONE"]
"#;

    let (stdout, stderr, status) = run_with_config(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "get",
            "Configured Get States",
            "--headings",
            "--no-content",
        ],
        config,
    );
    assert!(status.success(), "get failed:\n{stdout}\n{stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let headings = value["node"]["headings"].as_array().unwrap();

    assert_eq!(headings[0]["title"], "PKMS operational logging");
    assert!(headings[0]["todo_state"].is_null());
    assert_eq!(headings[1]["title"], "Configured task");
    assert_eq!(headings[1]["todo_state"], "NEXT");
}

#[test]
fn test_get_headings_text_with_uuids() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-text-heading-uuid.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       f0f0f0f0-f0f0-4f0f-8f0f-f0f0f0f0f0f0
:END:
#+title: Get Text Heading UUID

* Visible Heading
:PROPERTIES:
:ID:       f1f1f1f1-f1f1-4f1f-8f1f-f1f1f1f1f1f1
:END:
"#,
    )
    .unwrap();

    let (stdout, _stderr, status) = run(&[
        "--db",
        db,
        "get",
        "Get Text Heading UUID",
        "--headings",
        "--no-content",
    ]);
    assert!(status.success());
    assert!(stdout.contains("f1f1f1f1-f1f1-4f1f-8f1f-f1f1f1f1f1f1"));
    assert!(stdout.contains("Visible Heading"));
}

#[test]
fn test_get_heading_filters_text_content_to_heading_block() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-heading-filter-text.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       77777777-7777-4777-8777-777777777777
:END:
#+title: Get Heading Filter Text

Preamble should not be returned.
* Alpha
Alpha body.
* Beta
Beta body.
** Beta child
Child body.
* Gamma
Gamma body.
"#,
    )
    .unwrap();

    let (stdout, _stderr, status) = run(&[
        "--db",
        db,
        "get",
        "Get Heading Filter Text",
        "--heading",
        "Beta",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Note: Get Heading Filter Text"));
    assert!(stdout.contains("--- Content ---\n* Beta\nBeta body.\n** Beta child\nChild body.\n"));
    assert!(!stdout.contains("Preamble should not be returned."));
    assert!(!stdout.contains("Alpha body."));
    assert!(!stdout.contains("Gamma body."));
}

#[test]
fn test_get_heading_filters_json_content_to_heading_block() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-heading-filter-json.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       12121212-1212-4212-8212-121212121212
:END:
#+title: Get Heading Filter Json

* Alpha
Alpha body.
* Beta
Beta body.
** Beta child
Child body.
* Gamma
Gamma body.
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "get",
        "Get Heading Filter Json",
        "--heading",
        "Beta",
    ]);
    assert!(status.success());
    assert_eq!(v["node"]["title"], "Get Heading Filter Json");
    assert_eq!(
        v["node"]["content"],
        "* Beta\nBeta body.\n** Beta child\nChild body."
    );
}

#[test]
fn test_get_heading_returns_error_when_heading_missing() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-heading-filter-missing.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       99999999-9999-4999-8999-999999999999
:END:
#+title: Get Heading Filter Missing

* Present
Body.
"#,
    )
    .unwrap();

    let (_stdout, stderr, status) = run(&[
        "--db",
        db,
        "get",
        "Get Heading Filter Missing",
        "--heading",
        "Absent",
    ]);
    assert!(!status.success());
    assert!(stderr.contains("Heading not found: Absent"));
}
