use super::*;

fn tagged_db() -> TestDb {
    let db = TestDb::clean();
    db.write_roam(
        "one.org",
        r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: One
#+filetags: :alpha:common:

* Heading :common:
"#,
    );
    db.write_roam(
        "two.org",
        r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: Two
#+filetags: :alpha:

* TODO Task :beta:common:
"#,
    );
    db
}

#[test]
fn tags_lists_direct_assignments_by_usage_count() {
    let db = tagged_db();
    let (value, status) = db.run_json(&["tags"]);

    assert!(status.success());
    assert_eq!(
        value["tags"],
        serde_json::json!([
            {"tag": "common", "count": 3},
            {"tag": "alpha", "count": 2},
            {"tag": "beta", "count": 1}
        ])
    );
}

#[test]
fn tags_text_and_ndjson_display_counts() {
    let db = tagged_db();
    let (text, stderr, text_status) = db.run(&["tags"]);
    assert!(text_status.success(), "{stderr}");
    assert!(text.contains("TAG     COUNT"));
    assert!(text.contains("common  3"));

    let args = ["tags", "--output-format", "ndjson"];
    let (ndjson, stderr, ndjson_status) = db.run(&args);
    assert!(ndjson_status.success(), "{stderr}");
    let records = assert_ndjson_output(&args, &ndjson);
    assert_eq!(records.len(), 3);
    assert_eq!(records[0], serde_json::json!({"tag": "common", "count": 3}));
}
