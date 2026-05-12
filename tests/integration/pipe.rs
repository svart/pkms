use super::*;

#[test]
fn test_pipe_query_to_get() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &["--db", db, "--output-format", "ndjson", "query", "Note"],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "get",
            "--links",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe query|get failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_resolve_to_get() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "resolve",
            "--title",
            "Note",
        ],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "get",
            "--links",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe resolve|get failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_orphans_to_get() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &["--db", db, "--output-format", "ndjson", "orphans"],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "get",
            "--no-content",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe orphans|get failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_resolve_to_suggest() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "resolve",
            "--title",
            "Note",
        ],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "suggest",
            "--from-stdin",
            "--limit",
            "1",
        ],
    );
    assert!(
        status.success(),
        "pipe resolve|suggest failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("uuid").is_some(),
            "expected suggest output with uuid, got: {}",
            line
        );
        assert!(
            v.get("score").is_some(),
            "expected suggest output with score, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_resolve_to_validate() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "resolve",
            "--title",
            "Note",
        ],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "validate",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe resolve|validate failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("uuid").is_some(),
            "expected validate output, got: {}",
            line
        );
        assert!(
            v.get("healthy").is_some(),
            "expected healthy field, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_resolve_to_todo() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "resolve",
            "--title",
            "Agenda Item",
        ],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "todo",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe resolve|todo failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("uuid").is_some(),
            "expected todo output with uuid, got: {}",
            line
        );
    }
}

#[test]
fn test_pipe_suggest_to_get() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();
    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "suggest",
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            "--limit",
            "1",
        ],
        &[
            "--db",
            db,
            "--output-format",
            "ndjson",
            "get",
            "--no-content",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe suggest|get failed:\nstdout: {}\n",
        stdout
    );
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
    }
}
