#[test]
fn test_agenda_ids_match_open() {
    let (_dir, root) = super::setup_db();
    // Agenda JSON
    let (stdout, stderr, status) = super::run(&[
        "--db", root.to_str().unwrap(),
        "agenda", "--output-format", "json",
    ]);
    assert!(status.success(), "agenda failed: {stderr}");
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let items = json["items"].as_array().unwrap();
    for item in items {
        let id = item["id"].as_u64().unwrap();
        let (open_stdout, open_stderr, open_status) = super::run(&[
            "--db", root.to_str().unwrap(),
            "open", &id.to_string(),
        ]);
        assert!(
            open_status.success(),
            "open {id} failed: {open_stderr}\nstdout: {open_stdout}"
        );
    }
    eprintln!("All {} agenda IDs successfully opened", items.len());
}
