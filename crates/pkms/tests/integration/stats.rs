use super::*;

#[test]
fn test_stats_rejects_every_pair_of_modes() {
    let (_dir, root) = setup_clean_db();
    let modes: &[&[&str]] = &[&["--days", "7"], &["--hubs"], &["--tags"], &["--todos"]];

    for (index, left) in modes.iter().enumerate() {
        for right in &modes[index + 1..] {
            let mut args = vec!["--db", root.to_str().unwrap(), "stats"];
            args.extend_from_slice(left);
            args.extend_from_slice(right);
            let (stdout, stderr, status) = run(&args);
            assert!(
                !status.success(),
                "stats modes should conflict: {args:?}\nstdout: {stdout}\nstderr: {stderr}"
            );
        }
    }
}

#[test]
fn test_stats_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats"]);
    assert!(status.success());
    assert!(stdout.contains("Notes:"));
    assert!(stdout.contains("Links:"));
}

#[test]
fn test_stats_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
    ]);
    assert!(status.success());
    assert!(v.get("total_notes").is_some());
    assert!(v.get("directories").is_some());
}

#[test]
fn test_stats_orphans_exclude_dailies() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stats, status) = run_json(&["--db", db, "--output-format", "json", "stats"]);
    assert!(status.success());
    let (orphans, status) = run_json(&["--db", db, "--output-format", "json", "orphans"]);
    assert!(status.success());
    let (orphans_with_dailies, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "orphans",
        "--with-dailies",
    ]);
    assert!(status.success());

    assert_eq!(stats["orphans"], orphans["count"]);
    assert!(
        orphans_with_dailies["count"].as_u64().unwrap() > orphans["count"].as_u64().unwrap(),
        "test fixture should contain daily orphan notes"
    );
}

#[test]
fn test_stats_hubs() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--hubs"]);
    assert!(status.success());
}

#[test]
fn test_stats_hubs_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--hubs",
    ]);
    assert!(status.success());
    assert!(
        v.get("hubs")
            .and_then(|h| h.as_array())
            .is_some_and(|h| !h.is_empty())
    );
    assert!(v["hubs"][0]["uuid"].is_string());
}

#[test]
fn test_stats_tags() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--tags"]);
    assert!(status.success());
    assert!(stdout.contains("learning") || stdout.contains("emacs"));
}

#[test]
fn test_stats_tags_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--tags",
    ]);
    assert!(status.success());
    assert!(v["tags"].as_array().is_some_and(|t| !t.is_empty()));
    assert!(v["tags"][0]["tag"].is_string());
}

#[test]
fn test_stats_tags_counts_file_once_when_heading_ids_exist() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "tagged-heading.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Tagged Heading
#+filetags: :single:

* Heading
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
        "stats",
        "--tags",
    ]);
    assert!(status.success(), "stats --tags failed: {v}");
    let single = v["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["tag"] == "single")
        .unwrap();
    assert_eq!(single["count"], 1);
}

#[test]
fn test_stats_todos_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--todos"]);
    assert!(status.success());
    assert!(
        stdout.contains("TODO Statistics") || stdout.contains("Total TODO headings"),
        "stdout: {stdout}"
    );
}

#[test]
fn test_stats_todos_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--todos",
    ]);
    assert!(status.success());
    assert!(v.get("total_todo_headings").is_some());
    assert!(v.get("files_with_todos").is_some());
    assert!(v.get("by_state").is_some());
}

#[test]
fn test_stats_todos_counts_file_once_when_heading_ids_exist() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "todo-heading-id.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: TODO Heading ID

* TODO One task
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
        "stats",
        "--todos",
    ]);
    assert!(status.success(), "stats --todos failed: {v}");
    assert_eq!(v["total_todo_headings"], 1);
    assert_eq!(v["files_with_todos"], 1);
}

#[test]
fn test_stats_todos_uses_configured_states() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "configured-todo-states.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-8ccc-cccccccccccc
:END:
#+title: Configured TODO States

* API design
* NEXT Implement it
* DONE Verify it
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
            "stats",
            "--todos",
        ],
        config,
    );
    assert!(
        status.success(),
        "stats --todos failed:\n{stdout}\n{stderr}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["total_todo_headings"], 2);
    assert_eq!(value["files_with_todos"], 1);
    assert_eq!(
        value["by_state"],
        serde_json::json!([
            {"state": "DONE", "count": 1},
            {"state": "NEXT", "count": 1}
        ])
    );
}
