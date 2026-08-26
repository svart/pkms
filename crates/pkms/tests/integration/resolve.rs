use super::*;

#[test]
fn test_resolve_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "resolve", "--title", "Note"]);
    assert!(status.success());
    assert!(stdout.contains("Note A") || stdout.contains("Total:"));
}

#[test]
fn test_resolve_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    assert!(v.get("query").is_some());
    assert!(v.get("total").is_some());
    assert!(v.get("results").is_some());
    assert!(v["results"].as_array().is_some_and(|r| !r.is_empty()));
    assert!(v["results"][0]["uuid"].is_string());
}

#[test]
fn test_resolve_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

#[test]
fn test_resolve_query() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    assert!(
        v["total"].as_u64().unwrap_or(0) >= 1,
        "expected at least 1 result for 'Note', got {}",
        v["total"]
    );
}

#[test]
fn test_resolve_todos_uses_configured_states() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "not-a-task.org",
        r#":PROPERTIES:
:ID:       eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee
:END:
#+title: Not A Task

* API design
"#,
    );
    db_write(
        &root,
        "configured-task.org",
        r#":PROPERTIES:
:ID:       ffffffff-ffff-4fff-8fff-ffffffffffff
:END:
#+title: Configured Task

* NEXT Implement it
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
            "resolve",
            "--title",
            "",
            "--todos",
        ],
        config,
    );
    assert!(
        status.success(),
        "resolve --todos failed:\n{stdout}\n{stderr}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let titles: Vec<_> = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|note| note["title"].as_str())
        .collect();

    assert_eq!(titles, vec!["Configured Task"]);
}

#[test]
fn test_resolve_fields_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "resolve",
        "--title",
        "Note",
        "--fields",
        "uuid,title",
    ]);
    assert!(status.success());
    assert!(!stdout.contains("Tags:"));
}

#[test]
fn test_resolve_fields_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "resolve",
        "--title",
        "Note",
        "--fields",
        "uuid,title",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        for k in v.as_object().unwrap().keys() {
            assert!(k == "uuid" || k == "title", "unexpected key: {}", k);
        }
    }
}

#[test]
fn test_resolve_tags_matches_category() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--tags",
        "example",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Categorized Note"),
        "expected 'Categorized Note' to match via --tags 'example', got: {:?}",
        titles
    );
}

#[test]
fn test_resolve_heading_uuid() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("resolve-heading-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       33333333-3333-4333-8333-333333333333
:END:
#+title: Resolve Heading Test

* My Section
:PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444444
:END:
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--uuid",
        "44444444-4444-4444-8444-444444444444",
    ]);
    assert!(status.success(), "resolve failed: {:?}", v);
    assert_eq!(v["total"], 1, "should find 1 note by heading UUID");
    assert_eq!(v["results"][0]["title"], "Resolve Heading Test");
}
