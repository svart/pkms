use super::*;

fn item_by_title<'a>(output: &'a serde_json::Value, title: &str) -> &'a serde_json::Value {
    output["items"]
        .as_array()
        .expect("task output contains items")
        .iter()
        .find(|item| item["title"] == title)
        .unwrap_or_else(|| panic!("task output contains {title:?}"))
}

#[test]
fn task_identity_is_shared_by_list_agenda_show_open_and_mutation() {
    let db = TestDb::new().note_with_content(
        "20260710090000-contract-tasks.org",
        r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: Contract Tasks

* TODO Contract target
SCHEDULED: <2026-07-10 Fri>
* TODO Unscheduled sibling
"#,
    );

    let (list, list_status) = db.run_json(&["task", "list"]);
    assert!(list_status.success());
    let listed = item_by_title(&list, "Contract target");
    let source_id = listed["source_id"].as_str().expect("source ID");
    let display_id = listed["display_id"].as_str().expect("display ID");
    assert_eq!(display_id, format!("p{source_id}"));

    let (agenda, agenda_status) = db.run_json(&["task", "agenda"]);
    assert!(agenda_status.success());
    let agenda_item = item_by_title(&agenda, "Contract target");
    assert_eq!(agenda_item["source_id"], source_id);
    assert_eq!(agenda_item["display_id"], display_id);

    let (shown, show_status) = db.run_json(&["task", display_id, "show"]);
    assert!(show_status.success());
    assert_eq!(shown["heading_title"], "Contract target");
    let path = shown["path"].as_str().expect("show path");
    let line_number = shown["line_number"].as_u64().expect("show line");

    let (changed, change_status) =
        db.run_json(&["task", display_id, "state", "waiting", "--dry-run"]);
    assert!(change_status.success());
    assert_eq!(changed["path"], path);
    assert_eq!(changed["line_number"], line_number);
    assert_eq!(changed["old_state"], "TODO");
    assert_eq!(changed["new_state"], "WAITING");
    assert_eq!(changed["dry_run"], true);

    let (stdout, stderr, open_status) = db.run(&["task", display_id, "open", "--editor", "true"]);
    assert!(
        open_status.success(),
        "task open failed:\n{stdout}\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        format!("Opening: Contract Tasks (line {line_number})")
    );
}

#[test]
fn structured_stream_shapes_and_check_exit_status_are_stable() {
    let db = TestDb::new()
        .note(
            "tasks.org",
            "Contract Output",
            "22222222-2222-4222-8222-222222222222",
        )
        .task("tasks.org", "TODO", "Structured task");

    let (json, json_status) = db.run_json(&["task", "list"]);
    assert!(json_status.success());
    assert!(json.is_object());
    assert_eq!(json["total"], 1);
    assert_eq!(json["items"][0]["source"], "pkms");
    assert_eq!(json["items"][0]["source_id"], "1");
    assert_eq!(json["items"][0]["display_id"], "p1");

    let ndjson_args = ["--output-format", "ndjson", "task", "list"];
    let (stdout, stderr, ndjson_status) = db.run(&ndjson_args);
    assert!(
        ndjson_status.success(),
        "task NDJSON failed:\n{stdout}\n{stderr}"
    );
    let items = assert_ndjson_output(&ndjson_args, &stdout);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["source"], "pkms");
    assert_eq!(items[0]["source_id"], "1");
    assert_eq!(items[0]["display_id"], "p1");
    assert!(items[0].get("total").is_none());

    db.write_roam(
        "broken.org",
        r#":PROPERTIES:
:ID:       33333333-3333-4333-8333-333333333333
:END:
#+title: Broken Contract

[[id:44444444-4444-4444-8444-444444444444][Missing]]
"#,
    );

    let (check_json, check_status) = db.run_json(&["check"]);
    assert_eq!(check_status.code(), Some(1));
    assert_eq!(check_json["healthy"], false);
    assert_eq!(check_json["broken_links"].as_array().unwrap().len(), 1);

    let check_ndjson_args = ["--output-format", "ndjson", "check"];
    let (stdout, stderr, check_ndjson_status) = db.run(&check_ndjson_args);
    assert_eq!(
        check_ndjson_status.code(),
        Some(1),
        "check NDJSON status mismatch:\n{stdout}\n{stderr}"
    );
    let check_ndjson = assert_single_ndjson_object(&check_ndjson_args, &stdout);
    assert_eq!(check_ndjson["healthy"], false);
    assert_eq!(
        check_ndjson["broken_links"], check_json["broken_links"],
        "JSON and NDJSON check payloads must match"
    );
}
