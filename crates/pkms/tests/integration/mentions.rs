use super::*;

const RUST: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const SOURCE: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

fn mentions_db() -> TestDb {
    TestDb::new()
        .note_with_content(
            "rust.org",
            &format!(
                ":PROPERTIES:\n:ID:       {RUST}\n:ROAM_ALIASES: rustlang\n:END:\n#+title: Rust\n"
            ),
        )
        .note_with_content(
            "source.org",
            &format!(
                ":PROPERTIES:\n:ID:       {SOURCE}\n:END:\n#+title: Source\n\
                 Written in Rust, not [[id:{RUST}][Rust]].\n~rustlang~ and rustlang.\n"
            ),
        )
}

#[test]
fn mentions_ndjson_lists_outgoing_records() {
    let db = mentions_db();
    let args = ["--output-format", "ndjson", "mentions", SOURCE];

    let (stdout, stderr, status) = db.run(&args);

    assert!(status.success(), "stderr: {stderr}");
    let records = assert_ndjson_output(&args, &stdout);
    let summary: Vec<(u64, u64, &str, &str, bool)> = records
        .iter()
        .map(|r| {
            (
                r["line"].as_u64().unwrap(),
                r["col"].as_u64().unwrap(),
                r["phrase"].as_str().unwrap(),
                r["match"].as_str().unwrap(),
                r["already_linked"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (5, 12, "Rust", "title", true),
            (6, 16, "rustlang", "alias", true),
        ]
    );
    assert!(records.iter().all(|r| r["uuid"] == RUST));
    assert!(records.iter().all(|r| r["source_uuid"] == SOURCE));
}

#[test]
fn mentions_incoming_json_names_source_notes() {
    let db = mentions_db();

    let (output, status) = db.run_json(&["mentions", RUST, "--incoming"]);

    assert!(status.success());
    assert_eq!(output["mode"], "incoming");
    assert_eq!(output["target_uuid"], RUST);
    assert_eq!(output["count"], 2);
    assert!(
        output["mentions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["source_title"] == "Source")
    );
}

#[test]
fn mentions_reads_draft_text_from_stdin_dash() {
    let db = mentions_db();
    let root = db.root().to_str().unwrap();
    let args = ["--db", root, "--output-format", "json", "mentions", "-"];

    let (stdout, stderr, status) = run_with_stdin(&args, "A draft about rustlang.\n");

    assert!(status.success(), "stderr: {stderr}");
    let output = assert_json_object_output(&args, &stdout);
    assert_eq!(output["target"], serde_json::Value::Null);
    assert_eq!(output["mentions"][0]["uuid"], RUST);
    assert_eq!(
        output["mentions"][0]["source_uuid"],
        serde_json::Value::Null
    );
    assert_eq!(output["mentions"][0]["col"], 15);
}

#[test]
fn mentions_incoming_rejects_stdin_dash() {
    let db = mentions_db();
    let root = db.root().to_str().unwrap();

    let (_stdout, stderr, status) =
        run_with_stdin(&["--db", root, "mentions", "-", "--incoming"], "Rust");

    assert!(!status.success());
    assert!(stderr.contains("--incoming"), "stderr: {stderr}");
}

#[test]
fn mentions_text_output_shows_positions() {
    let db = mentions_db();

    let (stdout, stderr, status) = db.run(&["mentions", "Source"]);

    assert!(status.success(), "stderr: {stderr}");
    assert!(
        stdout.starts_with("Mentions in \"Source\" (2):\n"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "  5:12  \"Rust\" -> Rust ({RUST}) [already linked]"
        )),
        "stdout: {stdout}"
    );
}

#[test]
fn mentions_ndjson_pipes_mentioned_notes_into_get() {
    let db = mentions_db();
    let root = db.root().to_str().unwrap();

    let (stdout, status) = run_pipe_ndjson(
        &[
            "--db",
            root,
            "--output-format",
            "ndjson",
            "mentions",
            SOURCE,
        ],
        &[
            "--db",
            root,
            "--output-format",
            "ndjson",
            "get",
            "--from-stdin",
        ],
    );

    assert!(status.success(), "stdout: {stdout}");
    let records = assert_ndjson_output(&["mentions", "get"], &stdout);
    assert!(
        records.iter().all(|r| r["node"]["uuid"] == RUST),
        "stdout: {stdout}"
    );
}
