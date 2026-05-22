mod agenda;
mod all_commands;
mod check;
mod config;
mod context;
mod error;
mod fix;
mod get;
mod info;
mod new;
mod open;
mod orphans;
mod path;
mod pipe;
mod query;
mod resolve;
mod show;
mod snapshot;
mod stats;
mod suggest;
mod todo;
mod validate;

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

const TEST_CONFIG: &str = r#"[agenda]
open_todo_states = ["TODO", "IN-PROGRESS", "IDEA", "PROBLEM", "WAITING", "DELEGATED", "POSTPONED"]
closed_todo_states = ["DONE", "CANCELED"]
"#;

pub fn pkms_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pkms"))
}

fn setup_test_config_home() -> tempfile::TempDir {
    let config_home = tempfile::tempdir().unwrap();
    fs::write(config_home.path().join("pkms.toml"), TEST_CONFIG).unwrap();
    config_home
}

fn configure_test_command(command: &mut Command, config_home: &Path) {
    command
        .env("XDG_CONFIG_HOME", config_home)
        .env_remove("PKMS_DB_ROOT");
}

pub fn run(args: &[&str]) -> (String, String, ExitStatus) {
    let config_home = setup_test_config_home();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    let output = command.args(args).output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status,
    )
}

pub fn run_json(args: &[&str]) -> (serde_json::Value, ExitStatus) {
    let (stdout, _stderr, status) = run(args);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        panic!("No JSON output for args {:?}\nstdout: {}", args, stdout);
    }
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        panic!("Not JSON for args {:?}\nstdout: {}", args, stdout);
    }
    let v: serde_json::Value = serde_json::from_str(trimmed)
        .unwrap_or_else(|e| panic!("Invalid JSON for {:?}: {}\nError: {}", args, trimmed, e));
    (v, status)
}

fn run_pipe(producer_args: &[&str], consumer_args: &[&str]) -> (String, String, ExitStatus) {
    let producer_config_home = setup_test_config_home();
    let mut producer = Command::new(pkms_binary());
    configure_test_command(&mut producer, producer_config_home.path());
    let producer_output = producer
        .args(producer_args)
        .output()
        .expect("Failed to run producer");
    assert!(
        producer_output.status.success(),
        "Producer failed:\nargs: {:?}\nstdout: {}\nstderr: {}",
        producer_args,
        String::from_utf8_lossy(&producer_output.stdout),
        String::from_utf8_lossy(&producer_output.stderr),
    );
    let consumer_config_home = setup_test_config_home();
    let mut consumer = Command::new(pkms_binary());
    configure_test_command(&mut consumer, consumer_config_home.path());
    consumer.args(consumer_args);
    consumer
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = consumer.spawn().expect("Failed to spawn consumer");
    let mut stdin = child.stdin.take().expect("Failed to get consumer stdin");
    use std::io::Write;
    stdin
        .write_all(&producer_output.stdout)
        .expect("Failed to write to consumer stdin");
    drop(stdin);
    let output = child
        .wait_with_output()
        .expect("Failed to wait for consumer");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status,
    )
}

pub fn setup_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let roam = root.join("roam");
    let common = roam.join("common");
    let personal = roam.join("personal");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&personal).unwrap();

    write_linked_chain(&common);
    write_orphan(&personal);
    write_broken_link(&common);
    write_tagged(&common);
    write_aliased(&common);
    write_headings(&personal);
    write_file_links(&common);
    write_attachment_links(&common);
    write_duplicate_uuid(&personal);
    write_categorized(&common);
    write_bad_filetags(&personal);
    write_daily_note(&personal);
    write_daily_plan(&personal);
    write_agenda(&common);
    write_missing_agenda_tag(&common);
    write_nested_todos(&common);
    write_no_id_file(&root);

    (dir, root)
}

fn write_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

fn write_linked_chain(common: &std::path::Path) {
    write_file(
        common,
        "20220101000000-note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Note B]]
"#,
    );
    write_file(
        common,
        "20220101000001-note_b.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B

[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][Note C]]
"#,
    );
    write_file(
        common,
        "20220101000002-note_c.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note C

Content here.
"#,
    );
}

fn write_orphan(personal: &std::path::Path) {
    write_file(
        personal,
        "20220101000003-orphan.org",
        r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
#+title: Orphan Note

Alone.
"#,
    );
}

fn write_broken_link(common: &std::path::Path) {
    write_file(
        common,
        "20220101000004-broken.org",
        r#":PROPERTIES:
:ID:       eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee
:END:
#+title: Broken Note

[[id:ffffffff-ffff-4fff-ffff-ffffffffffff][Missing]]
"#,
    );
}

fn write_tagged(common: &std::path::Path) {
    write_file(
        common,
        "20220101000005-tagged.org",
        r#":PROPERTIES:
:ID:       55555555-5555-4555-5555-555555555555
:END:
#+title: Tagged Note
#+filetags: :learning:emacs:

Content with tags.
"#,
    );
}

fn write_aliased(common: &std::path::Path) {
    write_file(
        common,
        "20220101000006-aliased.org",
        r#":PROPERTIES:
:ID:       66666666-6666-4666-6666-666666666666
:ROAM_ALIASES: "AliasOne" "AliasTwo"
:END:
#+title: Aliased Note

Aliased content.
"#,
    );
}

fn write_headings(personal: &std::path::Path) {
    write_file(
        personal,
        "20220101000007-headings.org",
        r#":PROPERTIES:
:ID:       77777777-7777-4777-7777-777777777777
:END:
#+title: Headings Note

* Section 1
Text under section 1.
** Subsection 1.1
More text.
* Section 2
Final section.
"#,
    );
}

fn write_file_links(common: &std::path::Path) {
    write_file(
        common,
        "20220101000008-filelink.org",
        r#":PROPERTIES:
:ID:       88888888-8888-4888-8888-888888888888
:END:
#+title: File Link Note

[[file:~/docs/reference.pdf][Reference]]
[[file:relative/path.org][Relative]]
"#,
    );
}

fn write_attachment_links(common: &std::path::Path) {
    let root = common
        .parent()
        .and_then(|roam| roam.parent())
        .expect("common should be under roam under root");
    let attach_dir = root.join(".attach/aa/aaaaaa-aaaa-4aaa-aaaa-bbbbbbbbbbbb");
    fs::create_dir_all(&attach_dir).unwrap();
    fs::write(attach_dir.join("image.png"), b"image").unwrap();

    write_file(
        common,
        "20220101000010-attachment.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-bbbbbbbbbbbb
:END:
#+title: Attachment Link Note

[[attachment:image.png][Image]]
[[attachment:data/file.txt][Data File]]
"#,
    );
}

fn write_duplicate_uuid(personal: &std::path::Path) {
    write_file(
        personal,
        "20220101000009-duplicate.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Duplicate Title

This has same UUID as Note A.
"#,
    );
}

fn write_categorized(common: &std::path::Path) {
    write_file(
        common,
        "20220101000011-categorized.org",
        r#":PROPERTIES:
:ID:       99999999-9999-4999-9999-999999999999
:CATEGORY: example
:END:
#+title: Categorized Note

Content with a category.
"#,
    );
}

fn write_bad_filetags(personal: &std::path::Path) {
    write_file(
        personal,
        "20220101000012-badfiletags.org",
        r#":PROPERTIES:
:ID:       baadf00d-baad-4baa-dbaa-dbaadbaadbaa
:END:
#+title: Bad Filetags Note
#+filetags: :bad: :filetags:
#+filetags: :also::bad:
"#,
    );
}

fn write_daily_note(personal: &std::path::Path) {
    write_file(
        personal,
        "2024-06-15.org",
        r#":PROPERTIES:
:ID:       d1a1y1d1-d1a1-41d1-a1d1-d1a1d1a1d1a1
:END:
#+title: Daily Note
#+filetags: :daily:

* TODO Daily task
Some content
"#,
    );
}

fn write_daily_plan(personal: &std::path::Path) {
    write_file(
        personal,
        "2026-05-03.org",
        r#":PROPERTIES:
:ID:       e2e2e2e2-e2e2-4e2e-e2e2-e2e2e2e2e2e2
:END:
#+title: Daily Plan

* TODO Morning routine
SCHEDULED: <2026-05-03 Sun>
* IN-PROGRESS Project work
DEADLINE: <2026-05-05 Tue>
"#,
    );
}

fn write_agenda(common: &std::path::Path) {
    write_file(
        common,
        "20220101000013-agenda.org",
        r#":PROPERTIES:
:ID:       f3f3f3f3-f3f3-4f3f-f3f3-f3f3f3f3f3f3
:END:
#+title: Agenda Item
#+filetags: :agenda:

* TODO [#A] High priority task
SCHEDULED: <2026-05-10 Sun>
* TODO Low priority task
DEADLINE: <2026-06-15 Mon>
* DONE Completed task
"#,
    );
}

fn write_missing_agenda_tag(common: &std::path::Path) {
    write_file(
        common,
        "20220101000014-noagenda.org",
        r#":PROPERTIES:
:ID:       g4g4g4g4-g4g4-4g4g-g4g4-g4g4g4g4g4g4
:END:
#+title: Missing Agenda Tag

* TODO Fix this
SCHEDULED: <2026-05-10 Sun>
* WAITING Review
* IDEA Something
"#,
    );
}

fn write_nested_todos(common: &std::path::Path) {
    write_file(
        common,
        "20220101000015-nested-todo.org",
        r#":PROPERTIES:
:ID:       h5h5h5h5-h5h5-4h5h-h5h5-h5h5h5h5h5h5
:END:
#+title: Nested Todo Note
#+filetags: :project:

* TODO Parent task
SCHEDULED: <2026-06-01 Mon>
Some parent content.
** TODO Child task A
DEADLINE: <2026-06-10 Wed>
Child content.
** TODO Child task B
*** DONE Grandchild
* TODO Another top task
"#,
    );
}

fn write_no_id_file(root: &std::path::Path) {
    write_file(root, "no_id.org", "#+title: No ID\n");
}

pub fn setup_empty_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("empty")).unwrap();
    (dir, root)
}

pub fn setup_clean_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("db");
    fs::create_dir_all(root.join("roam")).unwrap();
    (dir, root)
}

pub fn db_write(root: &std::path::Path, name: &str, content: &str) {
    fs::write(root.join("roam").join(name), content).unwrap();
}

pub fn normalize_snapshot(output: &str, root: &std::path::Path) -> String {
    output.replace(root.to_str().unwrap(), "<DB_ROOT>")
}

pub fn run_pipe_ndjson(producer_args: &[&str], consumer_args: &[&str]) -> (String, ExitStatus) {
    let (stdout, _stderr, status) = run_pipe(producer_args, consumer_args);
    (stdout, status)
}
