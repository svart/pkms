use super::*;
#[cfg(feature = "web")]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(feature = "web")]
use std::net::TcpStream;
#[cfg(feature = "web")]
use std::process::{Command, Stdio};

#[test]
fn test_rag_help_works() {
    let (stdout, stderr, status) = run(&["rag", "--help"]);

    assert!(
        status.success(),
        "rag help should succeed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("status"));
    assert!(stdout.contains("retrieve"));
    assert!(stdout.contains("serve"));
}

#[test]
fn test_rag_status_json_empty_index() {
    let db = TestDb::clean();
    let rag_db = db.root().join("rag.sqlite3");
    let (value, status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "status",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);

    assert!(status.success());
    assert_eq!(value["notes"], 0);
    assert_eq!(value["chunks"], 0);
    assert_eq!(value["db_path"], rag_db.to_string_lossy().as_ref());
}

#[test]
fn test_rag_ingest_search_and_retrieve_json() {
    let db = TestDb::clean();
    let rag_db = db.root().join("rag.sqlite3");
    let fixture = rag_fixture();

    let (ingest, ingest_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "ingest",
        fixture.to_str().unwrap(),
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(ingest_status.success());
    assert_eq!(ingest["chunks_upserted"], 2);
    assert_eq!(ingest["embeddings_computed"], 2);

    let (search, search_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "search",
        "externalHostname",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(search_status.success());
    assert_eq!(search["query"], "externalHostname");
    assert_eq!(
        search["results"][0]["title"],
        "Media Library Migration to Jellyfin"
    );
    assert_eq!(
        search["results"][0]["uuid"],
        "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27"
    );
    assert!(search["results"][0].get("note_id").is_none());

    let (retrieve, retrieve_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "retrieve",
        "agenda inspect tasks",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(retrieve_status.success());
    assert_eq!(retrieve["query"], "agenda inspect tasks");
    assert_eq!(retrieve["results"][0]["title"], "PKMS Task Backend");
    assert_eq!(
        retrieve["results"][0]["uuid"],
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
    );
    assert!(retrieve["results"][0].get("note_id").is_none());
}

#[test]
fn test_rag_index_uses_db_root_as_notes_root_fallback() {
    let db = TestDb::clean();
    db.write_roam(
        "api-semantic-search.org",
        r#":PROPERTIES:
:ID:       eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee
:END:
#+title: API Semantic Search
* API usage
Agents call retrieve to search mounted PKMS notes.
"#,
    );
    let rag_db = db.root().join("rag.sqlite3");

    let (index_stdout, index_stderr, index_status) = run_hash(&[
        "--db",
        db.root().to_str().unwrap(),
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(
        index_status.success(),
        "index failed\nstdout: {index_stdout}\nstderr: {index_stderr}"
    );
    assert!(index_stdout.contains("Index phase: complete"));
    assert!(index_stdout.contains("Chunks: 1 seen"));

    let (search, search_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "search",
        "mounted PKMS",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(search_status.success());
    assert_eq!(
        search["results"][0]["uuid"],
        "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"
    );
    assert!(search["results"][0].get("note_id").is_none());
}

#[test]
fn test_rag_search_ndjson_pipes_to_task_list() {
    let db = TestDb::clean();
    db.write_roam(
        "distributed-mesh.org",
        r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
#+title: Distributed Mesh Project
* Context
Distributed mesh planning and execution notes.
* TODO Ship mesh routing milestone
"#,
    );
    let rag_db = db.root().join("rag.sqlite3");

    let (index_stdout, index_stderr, index_status) = run_hash(&[
        "--db",
        db.root().to_str().unwrap(),
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(
        index_status.success(),
        "index failed\nstdout: {index_stdout}\nstderr: {index_stderr}"
    );

    let (task_stdout, task_stderr, task_status) = run_pipe(
        &[
            "--db",
            db.root().to_str().unwrap(),
            "rag",
            "search",
            "distributed mesh",
            "--rag-db",
            rag_db.to_str().unwrap(),
            "--output-format",
            "ndjson",
        ],
        &[
            "--db",
            db.root().to_str().unwrap(),
            "--output-format",
            "json",
            "task",
            "list",
            "--from-stdin",
        ],
    );
    assert!(
        task_status.success(),
        "task list --from-stdin failed:\n{task_stdout}\n{task_stderr}"
    );
    let tasks: serde_json::Value = serde_json::from_str(task_stdout.trim()).unwrap();
    let items = tasks["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["note_uuid"],
        "dddddddd-dddd-4ddd-dddd-dddddddddddd"
    );
    assert_eq!(items[0]["title"], "Ship mesh routing milestone");
}

#[test]
fn test_rag_index_text_reports_progress_on_stderr() {
    let db = TestDb::clean();
    db.write_roam(
        "progress-rag.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Progress RAG Source
* Progress source
The database root feeds the progress-reporting RAG index.
"#,
    );
    let rag_db = db.root().join("rag.sqlite3");
    let config = format!(
        r#"
db_root = "{}"

[rag]
rag_db = "{}"

{TEST_CONFIG}
"#,
        db.root().display(),
        rag_db.display()
    );

    let (stdout, stderr, status) = run_hash_with_config(&["rag", "index"], &config);

    assert!(status.success());
    assert!(stdout.contains("Index phase: complete"));
    assert!(
        stderr.contains("RAG index: Processing records"),
        "stderr should include record progress\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("RAG index: Embedding chunks"),
        "stderr should include embedding progress\nstderr: {stderr}"
    );
}

#[test]
fn test_rag_index_uses_configured_rag_db_and_db_root_source() {
    let db = TestDb::clean();
    db.write_roam(
        "configured-rag.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Configured RAG Source
* Configured source
The configured database root feeds the RAG index.
"#,
    );
    let rag_db = db.root().join("configured-rag.sqlite3");
    let config = format!(
        r#"
db_root = "{}"

[rag]
rag_db = "{}"

{TEST_CONFIG}
"#,
        db.root().display(),
        rag_db.display()
    );

    let (index_stdout, index_stderr, index_status) =
        run_hash_with_config(&["rag", "index"], &config);
    assert!(
        index_status.success(),
        "index failed\nstdout: {index_stdout}\nstderr: {index_stderr}"
    );
    assert!(index_stdout.contains("Index phase: complete"));
    assert!(index_stdout.contains("Chunks: 1 seen"));

    let (search, search_status) = run_hash_json_with_config(
        &[
            "--output-format",
            "json",
            "rag",
            "search",
            "configured database root",
        ],
        &config,
    );
    assert!(search_status.success());
    assert_eq!(
        search["results"][0]["uuid"],
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
    );
    assert!(search["results"][0].get("note_id").is_none());
}

#[test]
fn test_rag_index_force_rebuild_removes_existing_sqlite_index() {
    let db = TestDb::clean();
    db.write_roam(
        "force-rebuild.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Force Rebuild Source
* Force rebuild
The RAG force rebuild path starts from an empty SQLite index.
"#,
    );
    let rag_db = db.root().join("rag.sqlite3");

    let (first_stdout, first_stderr, first_status) = run_hash(&[
        "--db",
        db.root().to_str().unwrap(),
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(
        first_status.success(),
        "first index failed\nstdout: {first_stdout}\nstderr: {first_stderr}"
    );

    let (second_stdout, second_stderr, second_status) = run_hash(&[
        "--db",
        db.root().to_str().unwrap(),
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(
        second_status.success(),
        "second index failed\nstdout: {second_stdout}\nstderr: {second_stderr}"
    );
    assert!(second_stdout.contains("Chunks: 1 seen, 0 upserted, 1 unchanged, 0 deleted"));
    assert!(second_stdout.contains("Embeddings: 0 computed, 0 skipped"));

    let (force_stdout, force_stderr, force_status) = run_hash(&[
        "--db",
        db.root().to_str().unwrap(),
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
        "--force-rebuild",
    ]);
    assert!(
        force_status.success(),
        "force index failed\nstdout: {force_stdout}\nstderr: {force_stderr}"
    );
    assert!(force_stdout.contains("Chunks: 1 seen, 1 upserted, 0 unchanged, 0 deleted"));
    assert!(force_stdout.contains("Embeddings: 1 computed, 0 skipped"));

    let (search, search_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "search",
        "empty SQLite index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(search_status.success());
    assert_eq!(
        search["results"][0]["uuid"],
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"
    );
    assert!(search["results"][0].get("note_id").is_none());
}

#[cfg(feature = "web")]
#[test]
fn test_rag_serve_note_route_uses_pkms_serve_viewer() {
    let db = TestDb::clean();
    db.write_roam(
        "rag-viewer.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: RAG Viewer Note

* Viewer section
RAG result titles open the rendered note viewer.
"#,
    );
    let rag_db = db.root().join("rag.sqlite3");
    let config_home = setup_test_config_home();
    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            db.root().to_str().unwrap(),
            "rag",
            "serve",
            "--rag-db",
            rag_db.to_str().unwrap(),
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .env_remove("PKMS_RAG_DB")
        .env("PKMS_RAG_EMBEDDING_PROVIDER", "hash")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms rag serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, _) = rest.split_once('/').unwrap();

    let response = http_get(host_port, "/?id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("<h1>RAG Viewer Note</h1>"));
    assert!(response.contains("data-preview-url=\"/preview\""));
    assert!(response.contains("Open in Emacs"));

    child.kill().unwrap();
    let _ = child.wait();
}

fn run_hash_json(args: &[&str]) -> (serde_json::Value, ExitStatus) {
    let (stdout, _stderr, status) = run_hash(args);
    let value = assert_json_output(args, &stdout);
    (value, status)
}

fn run_hash_json_with_config(args: &[&str], config: &str) -> (serde_json::Value, ExitStatus) {
    let (stdout, _stderr, status) = run_hash_with_config(args, config);
    let value = assert_json_output(args, &stdout);
    (value, status)
}

fn run_hash(args: &[&str]) -> (String, String, ExitStatus) {
    let config_home = setup_test_config_home();
    run_hash_with_config_home(args, config_home.path())
}

fn run_hash_with_config(args: &[&str], config: &str) -> (String, String, ExitStatus) {
    let config_home = tempfile::tempdir().unwrap();
    std::fs::write(config_home.path().join("pkms.toml"), config).unwrap();
    run_hash_with_config_home(args, config_home.path())
}

fn run_hash_with_config_home(
    args: &[&str],
    config_home: &std::path::Path,
) -> (String, String, ExitStatus) {
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home);
    let output = command
        .env_remove("PKMS_RAG_DB")
        .env_remove("PKMS_RAG_EMBEDDING_MODEL")
        .env("PKMS_RAG_EMBEDDING_PROVIDER", "hash")
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status,
    )
}

fn rag_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pkms-rag/tests/fixtures/retrieval-export.ndjson")
}

#[cfg(feature = "web")]
fn http_get(host_port: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(host_port).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}
