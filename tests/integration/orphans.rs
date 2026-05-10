use super::*;

#[test]
fn test_orphans_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "orphans"]);
    assert!(status.success());
    assert!(stdout.contains("Orphan Note"));
}

#[test]
fn test_orphans_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
    ]);
    assert!(status.success());
    assert!(
        v["count"].as_u64().unwrap_or(0) >= 1,
        "expected at least one orphan, got {}",
        v["count"]
    );
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Orphan Note"),
        "expected 'Orphan Note' in orphans, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_excludes_dailies_by_default() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        !titles.contains(&"Daily Note"),
        "expected 'Daily Note' excluded by default, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_with_dailies_includes_daily_notes() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
        "--with-dailies",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Daily Note"),
        "expected 'Daily Note' included with --with-dailies, got: {:?}",
        titles
    );
    assert!(
        titles.contains(&"Orphan Note"),
        "expected 'Orphan Note' still included, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "orphans",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}
