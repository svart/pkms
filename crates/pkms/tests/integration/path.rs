use super::*;

#[test]
fn test_path_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "path", "Note A", "Note C"]);
    assert!(status.success());
    assert!(stdout.contains("2 hop"));
}

#[test]
fn test_path_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "path",
        "Note A",
        "Note C",
    ]);
    assert!(status.success());
    assert_eq!(v["from"], "Note A");
    assert_eq!(v["to"], "Note C");
    assert_eq!(v["found"], true);
    assert_eq!(v["hops"], 2);
}

#[test]
fn test_path_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "path",
        "Nonexistent",
        "Note A",
    ]);
    assert!(!status.success());
}
