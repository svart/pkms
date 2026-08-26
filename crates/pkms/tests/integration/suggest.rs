use super::*;

#[test]
fn test_suggest_applies_scope_before_counting_and_limiting() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "target.org",
        ":PROPERTIES:\n:ID:       51515151-5151-4151-8151-515151515151\n:END:\n#+title: Scope Topic Target\n\nscope topic\n",
    );
    db_write(
        &root,
        "candidate.org",
        ":PROPERTIES:\n:ID:       61616161-6161-4161-8161-616161616161\n:END:\n#+title: Scope Topic Candidate\n#+filetags: :keep:\n\nscope topic\n",
    );
    db_write(
        &root,
        "2026-08-27.org",
        ":PROPERTIES:\n:ID:       71717171-7171-4171-8171-717171717171\n:END:\n#+title: Scope Topic Daily\n#+filetags: :keep:\n\nscope topic\n",
    );

    let (value, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "51515151-5151-4151-8151-515151515151",
        "--include-tags",
        "keep",
        "--without-dailies",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert_eq!(value["total"], 1);
    assert_eq!(value["showed"], 1);
    assert_eq!(value["suggestions"][0]["title"], "Scope Topic Candidate");
}

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
    if let Some(suggestions) = v["suggestions"].as_array()
        && !suggestions.is_empty()
    {
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

#[test]
fn test_suggest_defaults_to_ten_and_all_is_explicit() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "target.org",
        r#":PROPERTIES:
:ID:       78787878-7878-4878-8878-787878787878
:END:
#+title: Sharedtopic Target

sharedtopic content
"#,
    );
    for index in 0..12 {
        db_write(
            &root,
            &format!("candidate-{index}.org"),
            &format!(
                ":PROPERTIES:\n:ID:       00000000-0000-4000-8000-{index:012}\n:END:\n#+title: Sharedtopic Candidate {index}\n\nsharedtopic content\n"
            ),
        );
    }
    let db = root.to_str().unwrap();
    let target = "78787878-7878-4878-8878-787878787878";

    let (bounded, status) = run_json(&["--db", db, "--output-format", "json", "suggest", target]);
    assert!(status.success());
    assert_eq!(bounded["total"], 12);
    assert_eq!(bounded["showed"], 10);
    assert_eq!(bounded["suggestions"].as_array().unwrap().len(), 10);

    let (unbounded, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "suggest",
        target,
        "--all",
    ]);
    assert!(status.success());
    assert!(unbounded.get("showed").is_none());
    assert_eq!(unbounded["suggestions"].as_array().unwrap().len(), 12);
}

#[test]
fn test_suggest_rejects_limit_with_all() {
    let (_dir, root) = setup_clean_db();
    let (_stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "suggest",
        "78787878-7878-4878-8878-787878787878",
        "--limit",
        "2",
        "--all",
    ]);
    assert!(!status.success());
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
    assert_eq!(v["target"], "Special Topic");
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
