use super::*;

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
            .map_or(false, |h| !h.is_empty())
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
    assert!(v["tags"].as_array().map_or(false, |t| !t.is_empty()));
    assert!(v["tags"][0]["tag"].is_string());
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
