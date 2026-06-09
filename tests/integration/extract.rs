use super::*;

const NOTE_UUID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const HEADING_UUID: &str = "11111111-1111-4111-8111-111111111111";
const CHILD_UUID: &str = "22222222-2222-4222-8222-222222222222";

fn write_extract_fixture(db: &TestDb, root_properties: &str) -> std::path::PathBuf {
    let note_path = db.root().join("roam").join("source.org");
    db.write_roam(
        "source.org",
        &format!(
            r#":PROPERTIES:
:ID:       {NOTE_UUID}
:END:
#+title: Source Note

* Before
Before body.
** TODO [#A] Original Heading :tag:
:PROPERTIES:
:ID:       {HEADING_UUID}
{root_properties}:END:
SCHEDULED: <2026-06-09 Tue>
Body line.
#+begin_src rust
fn main() {{}}
#+end_src
*** Child With Own ID
:PROPERTIES:
:ID:       {CHILD_UUID}
:END:
Child body.
** After
After body.
"#
        ),
    );
    note_path
}

#[test]
fn test_extract_dry_run_does_not_write_files() {
    let db = TestDb::clean();
    let source_path = write_extract_fixture(&db, ":PROJECT: Alpha\n");
    let before = fs::read_to_string(&source_path).unwrap();

    let (stdout, _stderr, status) = db.run(&["extract", HEADING_UUID]);

    assert!(status.success(), "stdout: {stdout}");
    assert!(stdout.contains("dry-run"));
    assert!(stdout.contains("Original Heading"));
    assert_eq!(fs::read_to_string(&source_path).unwrap(), before);
    let roam_files = fs::read_dir(db.root().join("roam")).unwrap().count();
    assert_eq!(roam_files, 1, "dry-run should not create a note");
}

#[test]
fn test_extract_apply_creates_note_and_replaces_subtree() {
    let db = TestDb::clean();
    let source_path = write_extract_fixture(&db, ":PROJECT: Alpha\n");

    let (value, status) = db.run_json(&["extract", HEADING_UUID, "Better Note Title", "--apply"]);

    assert!(status.success(), "extract failed: {value}");
    assert_eq!(value["uuid"], HEADING_UUID);
    assert_eq!(value["title"], "Better Note Title");
    assert_eq!(value["created"], true);
    assert_eq!(value["applied"], true);
    assert_eq!(
        value["replacement"],
        format!("** TODO [#A] [[id:{HEADING_UUID}][Original Heading]] :tag:")
    );

    let source = fs::read_to_string(&source_path).unwrap();
    assert!(source.contains("* Before\nBefore body.\n"));
    assert!(source.contains(&format!(
        "** TODO [#A] [[id:{HEADING_UUID}][Original Heading]] :tag:\n** After"
    )));
    assert!(!source.contains("Body line."));
    assert!(!source.contains("Child With Own ID"));

    let new_path = value["new_path"].as_str().unwrap();
    let extracted = fs::read_to_string(new_path).unwrap();
    assert!(extracted.contains(&format!(
        ":ID:       {HEADING_UUID}\n:END:\n#+title: Better Note Title"
    )));
    assert!(extracted.contains("** TODO [#A] Original Heading :tag:"));
    assert!(extracted.contains(":PROJECT: Alpha"));
    assert!(!extracted.contains(&format!(":ID:       {HEADING_UUID}\n:PROJECT: Alpha")));
    assert!(extracted.contains(&format!(":ID:       {CHILD_UUID}")));
    assert!(extracted.contains("SCHEDULED: <2026-06-09 Tue>"));
    assert!(extracted.contains("#+begin_src rust"));
}

#[test]
fn test_extract_default_title_uses_heading_title() {
    let db = TestDb::clean();
    write_extract_fixture(&db, "");

    let (value, status) = db.run_json(&["extract", HEADING_UUID, "--apply"]);

    assert!(status.success(), "extract failed: {value}");
    assert_eq!(value["title"], "Original Heading");
    let extracted = fs::read_to_string(value["new_path"].as_str().unwrap()).unwrap();
    assert!(extracted.contains("#+title: Original Heading"));
}

#[test]
fn test_extract_removes_empty_root_properties_drawer() {
    let db = TestDb::clean();
    write_extract_fixture(&db, "");

    let (value, status) = db.run_json(&["extract", HEADING_UUID, "--apply"]);

    assert!(status.success(), "extract failed: {value}");
    let extracted = fs::read_to_string(value["new_path"].as_str().unwrap()).unwrap();
    assert!(extracted.contains("** TODO [#A] Original Heading :tag:\nSCHEDULED:"));
    assert!(!extracted.contains(&format!(
        "** TODO [#A] Original Heading :tag:\n:PROPERTIES:\n:ID:       {HEADING_UUID}\n:END:"
    )));
}

#[test]
fn test_extract_rejects_note_level_uuid() {
    let db = TestDb::clean();
    write_extract_fixture(&db, "");

    let (stdout, _stderr, status) = db.run(&["--output-format", "json", "extract", NOTE_UUID]);

    assert!(!status.success());
    let value = assert_json_error_output(&["extract", NOTE_UUID], &stdout);
    assert!(value["error"].as_str().unwrap().contains("note-level UUID"));
}

#[test]
fn test_extract_rejects_unknown_uuid() {
    let db = TestDb::clean();
    write_extract_fixture(&db, "");

    let unknown = "99999999-9999-4999-8999-999999999999";
    let (stdout, _stderr, status) = db.run(&["--output-format", "json", "extract", unknown]);

    assert!(!status.success());
    let value = assert_json_error_output(&["extract", unknown], &stdout);
    assert!(value["error"].as_str().unwrap().contains("not found"));
}

#[test]
fn test_extract_rejects_duplicate_uuid_state() {
    let db = TestDb::clean();
    write_extract_fixture(&db, "");
    db.write_roam(
        "duplicate-heading.org",
        &format!(
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb
:END:
#+title: Duplicate Heading

* Duplicate
:PROPERTIES:
:ID:       {HEADING_UUID}
:END:
"#
        ),
    );

    let (stdout, _stderr, status) = db.run(&[
        "--output-format",
        "json",
        "extract",
        HEADING_UUID,
        "--apply",
    ]);

    assert!(!status.success());
    let value = assert_json_error_output(&["extract", HEADING_UUID], &stdout);
    assert!(value["error"].as_str().unwrap().contains("Duplicate UUID"));
    let source = fs::read_to_string(db.root().join("roam").join("source.org")).unwrap();
    assert!(source.contains("Original Heading"));
}
