use super::*;

#[test]
fn test_rag_help_works() {
    let (stdout, stderr, status) = run(&["rag", "--help"]);

    assert!(
        status.success(),
        "rag help should succeed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("status"));
    assert!(stdout.contains("retrieve"));
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

    let (index, index_status) = run_hash_json(&[
        "--db",
        db.root().to_str().unwrap(),
        "--output-format",
        "json",
        "rag",
        "index",
        "--rag-db",
        rag_db.to_str().unwrap(),
    ]);
    assert!(index_status.success());
    assert_eq!(index["phase"], "complete");
    assert_eq!(index["chunks_seen"], 1);

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
        search["results"][0]["note_id"],
        "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"
    );
}

fn run_hash_json(args: &[&str]) -> (serde_json::Value, ExitStatus) {
    let (stdout, _stderr, status) = run_hash(args);
    let value = assert_json_output(args, &stdout);
    (value, status)
}

fn run_hash(args: &[&str]) -> (String, String, ExitStatus) {
    let config_home = setup_test_config_home();
    let mut command = std::process::Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command
        .env_remove("PKMS_RAG_DB")
        .env_remove("PKMS_RAG_NOTES_ROOT")
        .env_remove("PKMS_RAG_INDEX_SOURCE")
        .env_remove("PKMS_RAG_HOST")
        .env_remove("PKMS_RAG_PORT")
        .env_remove("PKMS_RAG_EMBEDDING_MODEL")
        .env_remove("PKMS_RAG_EMBEDDING_BATCH_SIZE")
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
