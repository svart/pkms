use super::*;

#[test]
fn test_new_dry_run() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "new", "Test Title"]);
    assert!(status.success());
    assert!(stdout.contains("Test Title"));
    assert!(stdout.contains("dry-run"));
}

#[test]
fn test_new_create() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "new",
        "Fresh Note",
        "--create",
    ]);
    assert!(status.success());
    assert!(stdout.contains("created"));
}

#[test]
fn test_new_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "new",
        "Test Note",
    ]);
    assert!(status.success());
    assert_eq!(v["title"], "Test Note");
    assert_eq!(v["created"], false);
    assert!(v.get("uuid").is_some());
}

#[test]
fn test_new_with_tags() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "new",
        "Tagged New",
        "--create",
        "--tags",
        "foo,bar",
    ]);
    assert!(status.success());
    assert_eq!(v["created"], true);
}

#[test]
fn test_new_with_multi_word_aliases() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "new",
        "Aliased New",
        "--create",
        "--aliases",
        "Alias One,Alias Two",
    ]);
    assert!(status.success(), "new failed: {v}");

    let path = v["path"].as_str().unwrap();
    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content.matches(":PROPERTIES:").count(), 1);
    assert!(content.contains(r#":ROAM_ALIASES: "Alias One" "Alias Two""#));

    let (resolved, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Alias One",
    ]);
    assert!(status.success(), "resolve failed: {resolved}");
    let titles: Vec<&str> = resolved["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Aliased New"),
        "expected new note to resolve by multi-word alias, got: {resolved}"
    );
}

#[test]
fn test_new_with_heading() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("test-heading-note.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: Test Heading Note

* My Heading
Some content
"#,
    )
    .unwrap();

    let (stdout, _stderr, status) = run(&[
        "--db",
        db,
        "new",
        "Test Heading Note",
        "--create",
        "--heading",
        "My Heading",
    ]);
    assert!(status.success(), "stdout: {}", stdout);
    assert!(stdout.contains("Heading UUID"));

    let content = std::fs::read_to_string(&note_path).unwrap();
    let id_count = content.matches(":ID:").count();
    assert_eq!(id_count, 2, "should have note-level and heading-level IDs");
}

#[test]
fn test_new_with_heading_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("json-heading-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: JSON Heading Test

* JSON Section
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "new",
        "JSON Heading Test",
        "--create",
        "--heading",
        "JSON Section",
    ]);
    assert!(status.success());
    assert!(v.get("heading").is_some());
    assert_eq!(v["uuid"], "22222222-2222-4222-8222-222222222222");
    assert_eq!(v["filename"], "json-heading-test.org");
    assert_eq!(v["path"], note_path.to_string_lossy().to_string());
    assert_eq!(v["heading"]["title"], "JSON Section");
    assert!(v["heading"]["uuid"].is_string());
}

#[test]
fn test_new_with_heading_matches_todo_heading_title() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let note_path = root.join("roam").join("common").join("todo-heading-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       33333333-3333-4333-8333-333333333333
:END:
#+title: TODO Heading Test

* TODO My Heading :work:
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "new",
        "TODO Heading Test",
        "--create",
        "--heading",
        "My Heading",
    ]);
    assert!(status.success(), "heading creation failed: {v}");
    assert_eq!(v["heading"]["title"], "My Heading");

    let content = fs::read_to_string(&note_path).unwrap();
    assert_eq!(content.matches(":ID:").count(), 2);
}

#[test]
fn test_new_with_heading_adds_id_to_existing_properties_drawer() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let note_path = root
        .join("roam")
        .join("common")
        .join("heading-existing-properties.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444444
:END:
#+title: Heading Properties Test

* Existing Properties
:PROPERTIES:
:PROJECT: Alpha
:END:
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "new",
        "Heading Properties Test",
        "--create",
        "--heading",
        "Existing Properties",
    ]);
    assert!(status.success(), "heading creation failed: {v}");

    let content = fs::read_to_string(&note_path).unwrap();
    assert_eq!(content.matches("* Existing Properties").count(), 1);
    assert_eq!(content.matches(":PROPERTIES:").count(), 2);
    assert!(content.contains(":PROJECT: Alpha\n:ID:"));
}
