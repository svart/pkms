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
    for v in assert_ndjson_output(&["query", "get"], &stdout) {
        assert!(v.get("node").is_some(), "expected get output, got: {}", v);
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
    for v in assert_ndjson_output(&["resolve", "get"], &stdout) {
        assert!(v.get("node").is_some(), "expected get output, got: {}", v);
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
    for v in assert_ndjson_output(&["orphans", "get"], &stdout) {
        assert!(v.get("node").is_some(), "expected get output, got: {}", v);
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
    for v in assert_ndjson_output(&["resolve", "suggest"], &stdout) {
        assert!(
            v.get("uuid").is_some(),
            "expected suggest output with uuid, got: {}",
            v
        );
        assert!(
            v.get("score").is_some(),
            "expected suggest output with score, got: {}",
            v
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
    for v in assert_ndjson_output(&["resolve", "validate"], &stdout) {
        assert!(
            v.get("uuid").is_some(),
            "expected validate output, got: {}",
            v
        );
        assert!(
            v.get("healthy").is_some(),
            "expected healthy field, got: {}",
            v
        );
    }
}

#[test]
fn test_pipe_resolve_to_task_list() {
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
            "task",
            "list",
            "--from-stdin",
        ],
    );
    assert!(
        status.success(),
        "pipe resolve|task list failed:\nstdout: {}\n",
        stdout
    );
    for v in assert_ndjson_output(&["resolve", "task", "list"], &stdout) {
        assert!(
            v.get("uuid").is_some(),
            "expected task list output with uuid, got: {}",
            v
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
    for v in assert_ndjson_output(&["suggest", "get"], &stdout) {
        assert!(v.get("node").is_some(), "expected get output, got: {}", v);
    }
}
