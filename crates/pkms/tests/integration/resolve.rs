use super::*;

#[test]
fn test_resolve_applies_shared_scope_filters() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "projects/keep.org",
        ":PROPERTIES:\n:ID:       14141414-1414-4414-8414-141414141414\n:END:\n#+title: Scoped Resolve\n#+filetags: :keep:\n",
    );
    db_write(
        &root,
        "archive/other.org",
        ":PROPERTIES:\n:ID:       15151515-1515-4515-8515-151515151515\n:END:\n#+title: Scoped Resolve\n#+filetags: :keep:\n",
    );

    let (value, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Scoped Resolve",
        "--include-tags",
        "keep",
        "--path-prefix",
        "roam/projects",
    ]);
    assert!(status.success());
    assert_eq!(value["total"], 1);
    assert_eq!(
        value["results"][0]["uuid"],
        "14141414-1414-4414-8414-141414141414"
    );
}

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

fn write_graph_notes(root: &std::path::Path) {
    db_write(
        root,
        "paragraph.org",
        ":PROPERTIES:\n:ID:       21212121-2121-4121-8121-212121212121\n:END:\n#+title: Paragraph\n",
    );
    db_write(
        root,
        "graph.org",
        ":PROPERTIES:\n:ID:       22222222-2222-4222-8222-222222222222\n:END:\n#+title: Graph\n",
    );
    db_write(
        root,
        "graph-theory.org",
        ":PROPERTIES:\n:ID:       23232323-2323-4323-8323-232323232323\n:END:\n#+title: Graph Theory\n",
    );
    db_write(
        root,
        "networks.org",
        ":PROPERTIES:\n:ID:       24242424-2424-4424-8424-242424242424\n:ROAM_ALIASES: \"graph\"\n:END:\n#+title: Networks\n",
    );
}

fn titles_and_kinds(value: &serde_json::Value) -> Vec<(String, String)> {
    value["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["title"].as_str().unwrap().to_string(),
                r["match_kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn test_resolve_title_match_modes() {
    let (_dir, root) = setup_clean_db();
    write_graph_notes(&root);
    let db = root.to_str().unwrap();

    let (value, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--title",
        "graph",
    ]);
    assert!(status.success());
    assert_eq!(
        titles_and_kinds(&value),
        vec![
            ("Graph".into(), "exact".into()),
            ("Networks".into(), "alias".into()),
            ("Graph Theory".into(), "word".into()),
            ("Paragraph".into(), "substring".into()),
        ]
    );

    let (value, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--title",
        "graph",
        "--word",
    ]);
    assert!(status.success());
    assert_eq!(value["total"], 3);
    assert!(!titles_and_kinds(&value).contains(&("Paragraph".into(), "substring".into())));

    let (value, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--title",
        "graph",
        "--exact",
    ]);
    assert!(status.success());
    assert_eq!(value["total"], 2);
}

#[test]
fn test_resolve_repeated_titles_ndjson_tags_each_query() {
    let (_dir, root) = setup_clean_db();
    write_graph_notes(&root);

    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "resolve",
        "--title",
        "networks",
        "--title",
        "graph",
        "--exact",
        "--fields",
        "uuid,matched_query,match_kind",
    ]);
    assert!(status.success());
    let rows: Vec<serde_json::Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let pairs: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r["matched_query"].as_str().unwrap(),
                r["uuid"].as_str().unwrap(),
                r["match_kind"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("networks", "24242424-2424-4424-8424-242424242424", "exact"),
            ("graph", "22222222-2222-4222-8222-222222222222", "exact"),
            ("graph", "24242424-2424-4424-8424-242424242424", "alias"),
        ]
    );
}
