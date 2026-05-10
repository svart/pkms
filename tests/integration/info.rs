use super::*;

#[test]
fn test_info_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "info"]);
    assert!(status.success());
    assert!(stdout.contains("db_root") || stdout.contains("config"));
}

#[test]
fn test_info_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "info",
    ]);
    assert!(status.success());
    assert!(v.get("config").is_some());
    assert!(v.get("config_path").is_some());
}
