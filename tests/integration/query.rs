use super::*;

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
