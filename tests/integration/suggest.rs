use super::*;

#[test]
fn test_suggest_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Suggestions") || stdout.contains("Note B"));
}

#[test]
fn test_suggest_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert_eq!(v["target"], "Note A");
    assert!(v.get("suggestions").is_some());
    if let Some(suggestions) = v["suggestions"].as_array() {
        if !suggestions.is_empty() {
            let s = &suggestions[0];
            assert!(
                s.get("scores").is_some(),
                "missing per-factor scores: {}",
                s
            );
            let scores = s["scores"].as_object().unwrap();
            assert!(!scores.is_empty(), "scores should not be empty: {}", s);
            for (_k, v) in scores {
                assert!(v.is_number(), "score value should be number: {}", v);
            }
        }
    }
}

#[test]
fn test_suggest_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "suggest", "Nonexistent"]);
    assert!(!status.success());
}

#[test]
fn test_suggest_exclude_orphans() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--exclude-orphans",
    ]);
    assert!(status.success());
    let suggestions = v["suggestions"].as_array().unwrap();
    assert!(
        suggestions
            .iter()
            .all(|s| s["uuid"] != "dddddddd-dddd-4ddd-dddd-dddddddddddd"),
        "orphans should be excluded, got: {v}"
    );
}

#[test]
fn test_suggest_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "should produce NDJSON output");
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some(), "each line should have uuid");
        assert!(v.get("score").is_some(), "each line should have score");
        assert!(
            v.get("target_uuid").is_some(),
            "each line should have target_uuid"
        );
    }
}

#[test]
fn test_suggest_with_heading_uuid() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Special Topic
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
Content about special topic
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    assert_eq!(v["target"], "Topic");
    let suggestions = v["suggestions"].as_array().unwrap();
    // The parent note should appear as a suggestion (content keyword overlap)
    assert!(
        !suggestions.is_empty(),
        "should have parent note as suggestion"
    );
    // The first suggestion's heading_context should be set
    assert!(
        suggestions[0].get("heading_context").is_some(),
        "heading_context should be present when targeting a heading UUID"
    );
}

#[test]
fn test_suggest_with_heading_uuid_has_context() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Special Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
Content about special topic
[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][ref]]
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note B

Some content
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    let suggestions = v["suggestions"].as_array().unwrap();
    assert!(!suggestions.is_empty(), "should have suggestions");
    let has_context = suggestions
        .iter()
        .any(|s| s.get("heading_context").and_then(|c| c.as_str()).is_some());
    assert!(
        has_context,
        "suggest should include heading_context when targeting a heading UUID"
    );
}
