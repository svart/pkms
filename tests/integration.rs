use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn pkms_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pkms"))
}

fn setup_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let roam = root.join("roam");
    let common = roam.join("common");
    let personal = roam.join("personal");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&personal).unwrap();

    // Note A -> links to B
    let note_a = r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Note B]]
"#;
    fs::write(common.join("20220101000000-note_a.org"), note_a).unwrap();

    // Note B -> links to C
    let note_b = r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B

[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][Note C]]
"#;
    fs::write(common.join("20220101000001-note_b.org"), note_b).unwrap();

    // Note C -> no outgoing links (orphan target)
    let note_c = r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note C

Content here.
"#;
    fs::write(common.join("20220101000002-note_c.org"), note_c).unwrap();

    // Orphan note (no links in or out)
    let orphan = r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
#+title: Orphan Note

Alone.
"#;
    fs::write(personal.join("20220101000003-orphan.org"), orphan).unwrap();

    // Note with broken link
    let broken = r#":PROPERTIES:
:ID:       eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee
:END:
#+title: Broken Note

[[id:ffffffff-ffff-4fff-ffff-ffffffffffff][Missing]]
"#;
    fs::write(common.join("20220101000004-broken.org"), broken).unwrap();

    // File without :ID: (should be skipped)
    fs::write(root.join("no_id.org"), "#+title: No ID\n").unwrap();

    (dir, root)
}

#[test]
fn test_check_healthy() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("check")
        .output()
        .unwrap();
    // Exit code 1 is expected when issues exist (broken links in test DB)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Notes:"), "stdout: {}", stdout);
    assert!(stdout.contains("Broken Note"), "stdout: {}", stdout);
}

#[test]
fn test_check_json() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("--json")
        .arg("check")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"healthy\":false") || stdout.contains("\"broken_links\""), "stdout: {}", stdout);
}

#[test]
fn test_validate_note() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("validate")
        .arg("Note A")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("healthy"));
}

#[test]
fn test_get_note() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("get")
        .arg("Note A")
        .arg("--depth")
        .arg("1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note B"));
}

#[test]
fn test_path() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("path")
        .arg("Note A")
        .arg("Note C")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note C"));
    assert!(stdout.contains("2 hop"));
}

#[test]
fn test_tags() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("tags")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn test_stats() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("stats")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Notes:"));
    assert!(stdout.contains("Links:"));
}

#[test]
fn test_hubs() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("hubs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hubs"));
}

#[test]
fn test_orphans() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("orphans")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Orphan Note"));
}

#[test]
fn test_broken() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("broken")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Broken Note"));
}

#[test]
fn test_context() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("context")
        .arg("Note A")
        .arg("--depth")
        .arg("1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note B"));
}

#[test]
fn test_query() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("query")
        .arg("Note")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note B"));
}

#[test]
fn test_new_dry_run() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("new")
        .arg("Test Title")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Test Title"));
    assert!(stdout.contains("dry-run"));
}

#[test]
fn test_new_create() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("new")
        .arg("Fresh Note")
        .arg("--create")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("created"));
}

#[test]
fn test_json_output() {
    let (_dir, root) = setup_db();
    let output = Command::new(pkms_binary())
        .arg("--db")
        .arg(&root)
        .arg("--json")
        .arg("stats")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with('{'));
}
