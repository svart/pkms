use super::*;

#[test]
fn test_query_todos_uses_configured_states() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "query-configured-states.org",
        r#":PROPERTIES:
:ID:       12121212-1212-4212-8212-121212121212
:END:
#+title: Query Configured States

* API design
"#,
    );
    db_write(
        &root,
        "query-configured-task.org",
        r#":PROPERTIES:
:ID:       34343434-3434-4434-8434-343434343434
:END:
#+title: Query Configured Task

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
            "query",
            "Configured",
            "--todos",
        ],
        config,
    );
    assert!(status.success(), "query failed:\n{stdout}\n{stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let titles: Vec<_> = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|note| note["title"].as_str())
        .collect();

    assert_eq!(titles, vec!["Query Configured Task"]);
}

#[test]
fn test_query_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "query", "Note"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
}

#[test]
fn test_query_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Note",
    ]);
    assert!(status.success());
    assert_eq!(v["query"], "Note");
    assert!(v["total_results"].as_u64().unwrap_or(0) >= 2);
    assert!(v["results"].as_array().is_some_and(|r| !r.is_empty()));
}

#[test]
fn test_query_bounds_content_matches_and_reports_total() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "bounded-query.org",
        r#":PROPERTIES:
:ID:       56565656-5656-4656-8656-565656565656
:END:
#+title: Bounded Query

needle one
needle two
needle three
needle four
needle five
"#,
    );

    let (bounded, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "needle",
        "--content",
    ]);
    assert!(status.success());
    assert_eq!(bounded["results"][0]["content_matches_total"], 5);
    assert_eq!(
        bounded["results"][0]["content_matches"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let (limited, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "needle",
        "--content",
        "--max-matches-per-note",
        "2",
    ]);
    assert!(status.success());
    assert_eq!(
        limited["results"][0]["content_matches"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let (unbounded, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "needle",
        "--content",
        "--all-matches",
    ]);
    assert!(status.success());
    assert_eq!(
        unbounded["results"][0]["content_matches"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}

#[test]
fn test_query_rejects_conflicting_or_zero_match_limits() {
    let (_dir, root) = setup_clean_db();
    let db = root.to_str().unwrap();

    let (_stdout, _stderr, conflict) = run(&[
        "--db",
        db,
        "query",
        "needle",
        "--max-matches-per-note",
        "2",
        "--all-matches",
    ]);
    assert!(!conflict.success());

    let (_stdout, _stderr, zero) =
        run(&["--db", db, "query", "needle", "--max-matches-per-note", "0"]);
    assert!(!zero.success());
}

#[test]
fn test_query_missing_terms_json_error() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v.get("error").is_some());
}

#[test]
fn test_query_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "query",
        "Note",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

#[test]
fn test_query_scope_tags() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "learning",
        "--tags",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Tagged Note"));
    assert!(stdout.contains("Matches: tag"));
}

#[test]
fn test_query_limit() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Note",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert!(
        v["total_results"].as_u64().unwrap() > 1,
        "total should reflect all matches"
    );
    assert_eq!(v["showed"], 1);
}

#[test]
fn test_query_content_only() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "Content",
        "--content",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("Content here") || stdout.contains("Content with tags"),
        "content search should find content lines, got: {stdout}"
    );
}

#[test]
fn test_query_content_uses_primary_note_identity_with_heading_ids() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "content-heading.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Primary Content Note
#+filetags: :contenttag:

This body has unique-primary-content-needle.

* Heading Node
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );

    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "unique-primary-content-needle",
        "--content",
    ]);
    assert!(status.success(), "query --content failed: {v}");
    assert_eq!(v["total_results"], 1);
    let result = &v["results"][0];
    assert_eq!(result["uuid"], "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(result["title"], "Primary Content Note");
    assert_eq!(
        result["path"],
        root.join("roam")
            .join("content-heading.org")
            .display()
            .to_string()
    );
    assert_eq!(result["filetags"], serde_json::json!(["contenttag"]));
}

#[test]
fn test_query_title_only() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Orphan",
        "--title",
    ]);
    assert!(status.success());
    let results = v["results"].as_array().unwrap();
    assert_eq!(
        results.len(),
        1,
        "title-only search for 'Orphan' should find only Orphan Note, got: {v}"
    );
    assert_eq!(results[0]["title"], "Orphan Note");
}

#[test]
fn test_query_no_results() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "zzzzzzzzzznonexistent",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("0") || stdout.contains("no results") || stdout.contains("Results: 0"),
        "no results should be reported, got: {stdout}"
    );
}

#[test]
fn test_query_content_merge() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Broken",
        "--content",
    ]);
    assert!(status.success());
    let results = v["results"].as_array().unwrap();
    assert!(
        results.iter().any(|r| r["title"] == "Broken Note"),
        "Broken Note should match via title+content, got: {v}"
    );
}

#[test]
fn test_query_content_text_truncation() {
    let (_dir, root) = setup_clean_db();
    let content = (1..=10)
        .map(|i| format!("Line {i} with the secret word"))
        .collect::<Vec<_>>()
        .join("\n");
    db_write(
        &root,
        "long_note.org",
        &format!(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Long Note

{content}
"#,
        ),
    );
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "secret",
        "--content",
    ]);
    assert!(status.success());
    assert!(stdout.contains("secret"), "should show content matches");
    assert!(
        stdout.contains("and 7 more") || stdout.contains("and 2 more") || stdout.contains("> Line"),
        "should mention truncated count or show lines, got: {stdout}"
    );
}
