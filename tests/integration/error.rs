use super::*;

#[test]
fn test_error_note_not_found_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "validate".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "get".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "suggest".into(),
            "Nonexistent".into(),
        ],
    ];
    for args in &cases {
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, _stderr, status) = run(&args_refs);
        assert!(!status.success(), "Expected failure for {:?}", args_refs);
        assert_json_error_output(&args_refs, &stdout);
    }
}

#[test]
fn test_error_path_not_found_json() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "path",
        "Nonexistent",
        "Note A",
    ]);
    assert!(!status.success());
    let v = assert_json_error_output(
        &[
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "path",
            "Nonexistent",
            "Note A",
        ],
        &stdout,
    );
    assert!(v.get("error").is_some());
}

#[test]
fn test_empty_db_stats() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
    ]);
    assert!(status.success());
    assert_eq!(v["total_notes"], 0);
}

#[test]
fn test_empty_db_check() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], true);
}

#[test]
fn test_empty_db_resolve() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "nothing",
    ]);
    assert!(status.success());
    assert_eq!(v["total"], 0);
}

#[test]
fn test_missing_db_json_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(pkms_binary())
        .args(["--output-format", "json", "stats"])
        .env("XDG_CONFIG_HOME", dir.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(!output.status.success());
    assert_json_error_output(&["--output-format", "json", "stats"], &stdout);
}

#[test]
fn test_json_error_exit_code() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "validate".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "get".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "suggest".into(),
            "DoesNotExist".into(),
        ],
    ];
    for args in &cases {
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, _stderr, status) = run(&args_refs);
        assert!(!status.success(), "Expected failure for {:?}", args_refs);
        assert_json_error_output(&args_refs, &stdout);
    }
}
