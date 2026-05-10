use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

fn pkms_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pkms"))
}

fn run(args: &[&str]) -> (String, String, ExitStatus) {
    let output = Command::new(pkms_binary()).args(args).output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status,
    )
}

fn run_json(args: &[&str]) -> (serde_json::Value, ExitStatus) {
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
    let producer_output = Command::new(pkms_binary())
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
    let mut consumer = Command::new(pkms_binary());
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

fn _assert_success(status: ExitStatus, args: &[&str], stdout: &str, stderr: &str) {
    assert!(
        status.success(),
        "args: {:?}\nstdout: {}\nstderr: {}",
        args,
        stdout,
        stderr
    );
}

fn setup_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let roam = root.join("roam");
    let common = roam.join("common");
    let personal = roam.join("personal");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&personal).unwrap();

    // Note A -- internal link to Note B
    fs::write(
        common.join("20220101000000-note_a.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Note B]]
"#,
    )
    .unwrap();

    // Note B -- internal link to Note C
    fs::write(
        common.join("20220101000001-note_b.org"),
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B

[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][Note C]]
"#,
    )
    .unwrap();

    // Note C -- no outgoing links
    fs::write(
        common.join("20220101000002-note_c.org"),
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note C

Content here.
"#,
    )
    .unwrap();

    // Orphan note -- no links in or out
    fs::write(
        personal.join("20220101000003-orphan.org"),
        r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
#+title: Orphan Note

Alone.
"#,
    )
    .unwrap();

    // Broken Note -- links to nonexistent UUID
    fs::write(
        common.join("20220101000004-broken.org"),
        r#":PROPERTIES:
:ID:       eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee
:END:
#+title: Broken Note

[[id:ffffffff-ffff-4fff-ffff-ffffffffffff][Missing]]
"#,
    )
    .unwrap();

    // Tagged Note -- has filetags
    fs::write(
        common.join("20220101000005-tagged.org"),
        r#":PROPERTIES:
:ID:       55555555-5555-4555-5555-555555555555
:END:
#+title: Tagged Note
#+filetags: :learning:emacs:

Content with tags.
"#,
    )
    .unwrap();

    // Aliased Note -- has roam aliases
    fs::write(
        common.join("20220101000006-aliased.org"),
        r#":PROPERTIES:
:ID:       66666666-6666-4666-6666-666666666666
:ROAM_ALIASES: "AliasOne" "AliasTwo"
:END:
#+title: Aliased Note

Aliased content.
"#,
    )
    .unwrap();

    // Headings Note -- has org headings
    fs::write(
        personal.join("20220101000007-headings.org"),
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
    )
    .unwrap();

    // File Link Note -- has file: links
    fs::write(
        common.join("20220101000008-filelink.org"),
        r#":PROPERTIES:
:ID:       88888888-8888-4888-8888-888888888888
:END:
#+title: File Link Note

[[file:~/docs/reference.pdf][Reference]]
[[file:relative/path.org][Relative]]
"#,
    )
    .unwrap();

    // Attachment Link Note -- has attachment: links
    fs::write(
        common.join("20220101000010-attachment.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-bbbbbbbbbbbb
:END:
#+title: Attachment Link Note

[[attachment:image.png][Image]]
[[attachment:data/file.txt][Data File]]
"#,
    )
    .unwrap();

    // Duplicate UUID note -- same UUID as Note A
    fs::write(
        personal.join("20220101000009-duplicate.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Duplicate Title

This has same UUID as Note A.
"#,
    )
    .unwrap();

    // Categorized Note -- has CATEGORY property
    fs::write(
        common.join("20220101000011-categorized.org"),
        r#":PROPERTIES:
:ID:       99999999-9999-4999-9999-999999999999
:CATEGORY: example
:END:
#+title: Categorized Note

Content with a category.
"#,
    )
    .unwrap();

    // Bad filetags note -- invalid format (whitespace-only segment, empty segment)
    fs::write(
        personal.join("20220101000012-badfiletags.org"),
        r#":PROPERTIES:
:ID:       baadf00d-baad-4baa-dbaa-dbaadbaadbaa
:END:
#+title: Bad Filetags Note
#+filetags: :bad: :filetags:
#+filetags: :also::bad:
"#,
    )
    .unwrap();

    // Daily note -- orphan (no incoming/outgoing links), filename matches YYYY-MM-DD
    fs::write(
        personal.join("2024-06-15.org"),
        r#":PROPERTIES:
:ID:       d1a1y1d1-d1a1-41d1-a1d1-d1a1d1a1d1a1
:END:
#+title: Daily Note
#+filetags: :daily:

* TODO Daily task
Some content
"#,
    )
    .unwrap();

    // Daily note with explicit SCHEDULED
    fs::write(
        personal.join("2026-05-03.org"),
        r#":PROPERTIES:
:ID:       e2e2e2e2-e2e2-4e2e-e2e2-e2e2e2e2e2e2
:END:
#+title: Daily Plan

* TODO Morning routine
SCHEDULED: <2026-05-03 Sun>
* IN-PROGRESS Project work
DEADLINE: <2026-05-05 Tue>
"#,
    )
    .unwrap();

    // Agenda tagged note with TODOs
    fs::write(
        common.join("20220101000013-agenda.org"),
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
    )
    .unwrap();

    // No-agenda note with TODOs (missing :agenda: tag)
    fs::write(
        common.join("20220101000014-noagenda.org"),
        r#":PROPERTIES:
:ID:       g4g4g4g4-g4g4-4g4g-g4g4-g4g4g4g4g4g4
:END:
#+title: Missing Agenda Tag

* TODO Fix this
SCHEDULED: <2026-05-10 Sun>
* WAITING Review
* IDEA Something
"#,
    )
    .unwrap();

    // Malformed file -- no UUID
    fs::write(root.join("no_id.org"), "#+title: No ID\n").unwrap();

    (dir, root)
}

fn setup_empty_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("empty")).unwrap();
    (dir, root)
}

// ----------------------------------------------------------------
// CHECK
// ----------------------------------------------------------------
#[test]
fn test_check_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check"]);
    assert!(!status.success());
    assert!(stdout.contains("Notes:"), "stdout: {}", stdout);
    assert!(stdout.contains("Broken Note"), "stdout: {}", stdout);
}

#[test]
fn test_check_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
    ]);
    assert!(!status.success());
    assert!(v.get("stats").is_some());
    assert_eq!(v["healthy"], false);
    assert!(v.get("broken_links").is_some());
    assert!(
        v.get("filetags_issues").is_some(),
        "expected filetags_issues field"
    );
    let ft_count = v["filetags_issues"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        ft_count >= 1,
        "expected at least 1 filetags issue, got {}",
        ft_count
    );
    let dups = v["duplicates"]
        .get("duplicate_uuids")
        .and_then(|a| a.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(dups >= 1, "expected duplicate UUIDs");
}

// ----------------------------------------------------------------
// VALIDATE
// ----------------------------------------------------------------
#[test]
fn test_validate_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "validate", "Note A"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("healthy"));
}

#[test]
fn test_validate_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note A",
    ]);
    assert!(status.success());
    assert_eq!(v["title"], "Note A");
    assert_eq!(v["healthy"], true);
    assert!(v.get("uuid").is_some());
    assert!(v.get("outgoing").is_some());
    assert!(v.get("incoming").is_some());
}

#[test]
fn test_validate_broken_note() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Broken Note",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], false);
    assert!(v["broken_internal"].as_array().map_or(0, |a| a.len()) >= 1);
}

#[test]
fn test_validate_note_not_found() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "validate",
        "NonexistentNote",
    ]);
    assert!(!status.success());
    assert!(
        stderr.contains("not found") || stdout.contains("error"),
        "stderr: {}\nstdout: {}",
        stderr,
        stdout
    );
}

// ----------------------------------------------------------------
// STATS
// ----------------------------------------------------------------
#[test]
fn test_stats_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats"]);
    assert!(status.success());
    assert!(stdout.contains("Notes:"));
    assert!(stdout.contains("Links:"));
}

#[test]
fn test_stats_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
    ]);
    assert!(status.success());
    assert!(v.get("total_notes").is_some());
    assert!(v.get("directories").is_some());
}

// ----------------------------------------------------------------
// ORPHANS
// ----------------------------------------------------------------
#[test]
fn test_orphans_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "orphans"]);
    assert!(status.success());
    assert!(stdout.contains("Orphan Note"));
}

#[test]
fn test_orphans_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
    ]);
    assert!(status.success());
    assert!(
        v["count"].as_u64().unwrap_or(0) >= 1,
        "expected at least one orphan, got {}",
        v["count"]
    );
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Orphan Note"),
        "expected 'Orphan Note' in orphans, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_excludes_dailies_by_default() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        !titles.contains(&"Daily Note"),
        "expected 'Daily Note' excluded by default, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_with_dailies_includes_daily_notes() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "orphans",
        "--with-dailies",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["orphans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Daily Note"),
        "expected 'Daily Note' included with --with-dailies, got: {:?}",
        titles
    );
    assert!(
        titles.contains(&"Orphan Note"),
        "expected 'Orphan Note' still included, got: {:?}",
        titles
    );
}

#[test]
fn test_orphans_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "orphans",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

// ----------------------------------------------------------------
// STATS
// ----------------------------------------------------------------
#[test]
fn test_stats_hubs() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--hubs"]);
    assert!(status.success());
}

#[test]
fn test_stats_hubs_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--hubs",
    ]);
    assert!(status.success());
    assert!(
        v.get("hubs")
            .and_then(|h| h.as_array())
            .map_or(false, |h| !h.is_empty())
    );
    assert!(v["hubs"][0]["uuid"].is_string());
}

// ----------------------------------------------------------------
// CONTEXT
// ----------------------------------------------------------------
#[test]
fn test_context_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "context",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success(), "stdout: {}", stdout);
    assert!(stdout.contains("Note A"), "stdout: {}", stdout);
}

#[test]
fn test_context_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "context",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success());
    assert_eq!(v["target"], "Note A");
    assert!(v.get("context").is_some());
    assert!(v.get("estimated_tokens").is_some());
    assert!(v["context"].as_str().unwrap_or("").contains("Note B"));
}

#[test]
fn test_context_depth_2() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "context",
        "Note A",
        "--depth",
        "2",
    ]);
    assert!(status.success());
    assert!(v["context"].as_str().unwrap_or("").contains("Note C"));
}

#[test]
fn test_context_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "context", "Nonexistent"]);
    assert!(!status.success());
}

// ----------------------------------------------------------------
// RESOLVE
// ----------------------------------------------------------------
#[test]
fn test_resolve_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "resolve", "--title", "Note"]);
    assert!(status.success());
    assert!(stdout.contains("Note A") || stdout.contains("Total:"));
}

#[test]
fn test_resolve_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    assert!(v.get("query").is_some());
    assert!(v.get("total").is_some());
    assert!(v.get("results").is_some());
    assert!(v["results"].as_array().map_or(false, |r| !r.is_empty()));
    assert!(v["results"][0]["uuid"].is_string());
}

#[test]
fn test_resolve_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

#[test]
fn test_resolve_query() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "Note",
    ]);
    assert!(status.success());
    assert!(
        v["total"].as_u64().unwrap_or(0) >= 1,
        "expected at least 1 result for 'Note', got {}",
        v["total"]
    );
}

#[test]
fn test_resolve_fields_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "resolve",
        "--title",
        "Note",
        "--fields",
        "uuid,title",
    ]);
    assert!(status.success());
    assert!(!stdout.contains("Tags:"));
}

#[test]
fn test_resolve_fields_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "resolve",
        "--title",
        "Note",
        "--fields",
        "uuid,title",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        for k in v.as_object().unwrap().keys() {
            assert!(k == "uuid" || k == "title", "unexpected key: {}", k);
        }
    }
}

// ----------------------------------------------------------------
// FIX (dry run)
// ----------------------------------------------------------------
#[test]
fn test_fix_dry_run() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Would fix") || stdout.contains("use --apply"));
}

#[test]
fn test_fix_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert_eq!(v["broken_uuid"], "ffffffff-ffff-4fff-ffff-ffffffffffff");
    assert_eq!(v["applied"], false);
}

#[test]
fn test_fix_broken_not_found() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "00000000-0000-0000-0000-000000000000",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    // fix succeeds (0 replacements found); check output mentions 0
    assert!(status.success());
    assert!(stdout.contains("0 broken link") || stdout.contains("Would fix"));
}

#[test]
fn test_fix_apply_human_output() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "fix",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--apply",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("Fixed"),
        "Applied fix human output should say 'Fixed', got: {stdout}"
    );
    assert!(
        stdout.contains("broken link"),
        "Output should mention link count, got: {stdout}"
    );
}

#[test]
fn test_fix_apply_json_output() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "fix",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--apply",
    ]);
    assert!(status.success());
    assert_eq!(v["applied"], true, "fix should be applied, got: {v}");
}

// ----------------------------------------------------------------
// SUGGEST
// ----------------------------------------------------------------
#[test]
fn test_suggest_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Suggestions") || stdout.contains("Note B"));
}

#[test]
fn test_suggest_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    assert_eq!(v["target"], "Note A");
    assert!(v.get("suggestions").is_some());
    // Verify per-factor scores exist
    if let Some(suggestions) = v["suggestions"].as_array() {
        if !suggestions.is_empty() {
            let s = &suggestions[0];
            assert!(
                s.get("scores").is_some(),
                "missing per-factor scores: {}",
                s
            );
            let scores = s["scores"].as_object().unwrap();
            // At least one scoring factor should be present
            assert!(!scores.is_empty(), "scores should not be empty: {}", s);
            // Each score should be a number
            for (_k, v) in scores {
                assert!(v.is_number(), "score value should be number: {}", v);
            }
        }
    }
}

#[test]
fn test_suggest_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "suggest", "Nonexistent"]);
    assert!(!status.success());
}

#[test]
fn test_suggest_exclude_orphans() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
        "--exclude-orphans",
    ]);
    assert!(status.success());
    let suggestions = v["suggestions"].as_array().unwrap();
    // The orphan note (dddddddd-dddd-4ddd-dddd-dddddddddddd) should not appear
    assert!(
        suggestions
            .iter()
            .all(|s| s["uuid"] != "dddddddd-dddd-4ddd-dddd-dddddddddddd"),
        "orphans should be excluded, got: {v}"
    );
}

#[test]
fn test_suggest_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "suggest",
        "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    ]);
    assert!(status.success());
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "should produce NDJSON output");
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some(), "each line should have uuid");
        assert!(v.get("score").is_some(), "each line should have score");
        assert!(
            v.get("target_uuid").is_some(),
            "each line should have target_uuid"
        );
    }
}

// ----------------------------------------------------------------
// NEW
// ----------------------------------------------------------------
#[test]
fn test_new_dry_run() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "new", "Test Title"]);
    assert!(status.success());
    assert!(stdout.contains("Test Title"));
    assert!(stdout.contains("dry-run"));
}

#[test]
fn test_new_create() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "new",
        "Fresh Note",
        "--create",
    ]);
    assert!(status.success());
    assert!(stdout.contains("created"));
}

#[test]
fn test_new_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "new",
        "Test Note",
    ]);
    assert!(status.success());
    assert_eq!(v["title"], "Test Note");
    assert_eq!(v["created"], false);
    assert!(v.get("uuid").is_some());
}

#[test]
fn test_new_with_tags() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "new",
        "Tagged New",
        "--create",
        "--tags",
        "foo,bar",
    ]);
    assert!(status.success());
    assert_eq!(v["created"], true);
}

// ----------------------------------------------------------------
// GET
// ----------------------------------------------------------------
#[test]
fn test_get_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "get", "Note A", "--links"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
    assert!(stdout.contains("Note B"));
}

#[test]
fn test_get_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "get",
        "Note A",
        "--links",
    ]);
    assert!(status.success());
    assert_eq!(v["node"]["title"], "Note A");
    assert!(v.get("neighbors").is_some());
}

#[test]
fn test_get_no_content() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "get",
        "Note A",
        "--no-content",
    ]);
    assert!(status.success());
    assert!(!stdout.contains("--- Content ---"));
}

#[test]
fn test_get_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "get", "Nonexistent"]);
    assert!(!status.success());
}

// ----------------------------------------------------------------
// QUERY
// ----------------------------------------------------------------
#[test]
fn test_query_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "query", "Note"]);
    assert!(status.success());
    assert!(stdout.contains("Note A"));
}

#[test]
fn test_query_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Note",
    ]);
    assert!(status.success());
    assert_eq!(v["query"], "Note");
    assert!(v["total_results"].as_u64().unwrap_or(0) >= 2);
    assert!(v["results"].as_array().map_or(false, |r| !r.is_empty()));
}

#[test]
fn test_query_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "query",
        "Note",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

#[test]
fn test_query_scope_tags() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "learning",
        "--tags",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Tagged Note"));
    assert!(stdout.contains("Matches: tag"));
}

#[test]
fn test_query_limit() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Note",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert!(
        v["total_results"].as_u64().unwrap() > 1,
        "total should reflect all matches"
    );
    assert_eq!(v["showed"], 1);
}

#[test]
fn test_query_content_only() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "Content",
        "--content",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("Content here") || stdout.contains("Content with tags"),
        "content search should find content lines, got: {stdout}"
    );
}

#[test]
fn test_query_title_only() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Orphan",
        "--title",
    ]);
    assert!(status.success());
    let results = v["results"].as_array().unwrap();
    assert_eq!(
        results.len(),
        1,
        "title-only search for 'Orphan' should find only Orphan Note, got: {v}"
    );
    assert_eq!(results[0]["title"], "Orphan Note");
}

#[test]
fn test_query_no_results() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "zzzzzzzzzznonexistent",
    ]);
    assert!(status.success());
    assert!(
        stdout.contains("0") || stdout.contains("no results") || stdout.contains("Results: 0"),
        "no results should be reported, got: {stdout}"
    );
}

#[test]
fn test_query_content_merge() {
    let (_dir, root) = setup_db();
    // Search for something that appears as both title match and content match
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "query",
        "Broken",
        "--content",
    ]);
    assert!(status.success());
    let results = v["results"].as_array().unwrap();
    assert!(
        results.iter().any(|r| r["title"] == "Broken Note"),
        "Broken Note should match via title+content, got: {v}"
    );
}

#[test]
fn test_query_content_text_truncation() {
    let (_dir, root) = setup_clean_db();
    // Create a note with many lines containing the search term
    let content = (1..=10)
        .map(|i| format!("Line {i} with the secret word"))
        .collect::<Vec<_>>()
        .join("\n");
    db_write(
        &root,
        "long_note.org",
        &format!(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Long Note

{content}
"#,
        ),
    );
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "secret",
        "--content",
    ]);
    assert!(status.success());
    // Should show content matches (at most 3 visible + "... and N more")
    assert!(stdout.contains("secret"), "should show content matches");
    assert!(
        stdout.contains("and 7 more") || stdout.contains("and 2 more") || stdout.contains("> Line"),
        "should mention truncated count or show lines, got: {stdout}"
    );
}

// ----------------------------------------------------------------
// INFO
// ----------------------------------------------------------------
#[test]
fn test_info_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "info"]);
    assert!(status.success());
    assert!(stdout.contains("db_root") || stdout.contains("config"));
}

#[test]
fn test_info_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "info",
    ]);
    assert!(status.success());
    assert!(v.get("config").is_some());
    assert!(v.get("config_path").is_some());
}

// ----------------------------------------------------------------
// PATH
// ----------------------------------------------------------------
#[test]
fn test_path_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "path", "Note A", "Note C"]);
    assert!(status.success());
    assert!(stdout.contains("2 hop"));
}

#[test]
fn test_path_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "path",
        "Note A",
        "Note C",
    ]);
    assert!(status.success());
    assert_eq!(v["from"], "Note A");
    assert_eq!(v["to"], "Note C");
    assert_eq!(v["found"], true);
    assert_eq!(v["hops"], 2);
}

#[test]
fn test_path_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "path",
        "Nonexistent",
        "Note A",
    ]);
    assert!(!status.success());
}

// ----------------------------------------------------------------
// CATEGORY
// ----------------------------------------------------------------
#[test]
fn test_resolve_tags_matches_category() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--tags",
        "example",
    ]);
    assert!(status.success());
    let titles: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Categorized Note"),
        "expected 'Categorized Note' to match via --tags 'example', got: {:?}",
        titles
    );
}

// ----------------------------------------------------------------
// FILETAGS VALIDATION
// ----------------------------------------------------------------
#[test]
fn test_validate_bad_filetags() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Bad Filetags Note",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], false);
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .filter(|i| i.contains("filetags"))
        .collect();
    assert!(
        issues.len() >= 2,
        "expected at least 2 filetags issues, got {:?}",
        issues
    );
}

// ----------------------------------------------------------------
// TAGS
// ----------------------------------------------------------------
#[test]
fn test_stats_tags() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--tags"]);
    assert!(status.success());
    assert!(stdout.contains("learning") || stdout.contains("emacs"));
}

#[test]
fn test_stats_tags_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--tags",
    ]);
    assert!(status.success());
    assert!(v["tags"].as_array().map_or(false, |t| !t.is_empty()));
    assert!(v["tags"][0]["tag"].is_string());
}

// ----------------------------------------------------------------
// FILE LINK CHECK
// ----------------------------------------------------------------
#[test]
fn test_check_file_links_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check", "--file-links"]);
    assert!(!status.success());
    assert!(stdout.contains("Broken files:"));
    assert!(stdout.contains("File Link Note"));
}

#[test]
fn test_check_file_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
    ]);
    assert!(!status.success());
    assert!(
        v.get("broken_file_links").is_some(),
        "expected broken_file_links field"
    );
    let broken_files = v["broken_file_links"].as_array().unwrap();
    assert!(broken_files.len() >= 1, "expected broken file links");
    assert!(broken_files[0]["source_title"].is_string());
    assert!(broken_files[0]["target_path"].is_string());
    assert!(
        v.get("stats").is_none(),
        "stats should not appear with --file-links only"
    );
    assert!(
        v.get("broken_links").is_none(),
        "broken_links should not appear with --file-links only"
    );
}

// ----------------------------------------------------------------
// ATTACHMENT LINK CHECK
// ----------------------------------------------------------------
#[test]
fn test_check_attachment_links_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "check",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(stdout.contains("Broken attach:"));
    assert!(stdout.contains("Attachment Link Note"));
}

#[test]
fn test_check_attachment_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(
        v.get("broken_attachment_links").is_some(),
        "expected broken_attachment_links field"
    );
    let broken_attach = v["broken_attachment_links"].as_array().unwrap();
    assert!(broken_attach.len() >= 1, "expected broken attachment links");
    assert!(broken_attach[0]["source_title"].is_string());
    assert!(broken_attach[0]["target_path"].is_string());
    assert!(
        v.get("stats").is_none(),
        "stats should not appear with --attachment-links only"
    );
}

#[test]
fn test_check_file_and_attachment_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--file-links",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(v.get("broken_file_links").is_some());
    assert!(v.get("broken_attachment_links").is_some());
    assert!(v["broken_file_links"].as_array().unwrap().len() >= 1);
    assert!(v["broken_attachment_links"].as_array().unwrap().len() >= 1);
    assert!(
        v.get("stats").is_none(),
        "stats should not appear without --stats flag"
    );
}

#[test]
fn test_check_id_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    assert!(!status.success());
    assert!(
        v["broken_links"]
            .as_array()
            .map_or(false, |a| !a.is_empty())
    );
    assert!(v["broken_links"][0]["source_uuid"].is_string());
    // file and attachment checks should be absent when only --id-links
    assert!(
        v.get("broken_file_links").is_none(),
        "expected no file links with --id-links only"
    );
    assert!(
        v.get("broken_attachment_links").is_none(),
        "expected no attachment links with --id-links only"
    );
}

// ----------------------------------------------------------------
// ERROR PATH: note not found (with --output-format json, expect structured error)
// ----------------------------------------------------------------
#[test]
fn test_error_note_not_found_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "validate".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "get".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "suggest".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "context".into(),
            "Nonexistent".into(),
        ],
    ];
    for args in &cases {
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, _stderr, status) = run(&args_refs);
        assert!(!status.success(), "Expected failure for {:?}", args_refs);
        let trimmed = stdout.trim();
        assert!(
            trimmed.starts_with('{'),
            "Expected JSON error for {:?}, got: {}",
            args_refs,
            stdout
        );
        let v: serde_json::Value = serde_json::from_str(trimmed)
            .unwrap_or_else(|_| panic!("Invalid JSON for {:?}: {}", args_refs, stdout));
        assert!(
            v.get("error").is_some(),
            "Missing error key for {:?}",
            args_refs
        );
    }
}

#[test]
fn test_error_path_not_found_json() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "path",
        "Nonexistent",
        "Note A",
    ]);
    assert!(!status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v.get("error").is_some());
}

// ----------------------------------------------------------------
// ERROR PATH: empty database
// ----------------------------------------------------------------
#[test]
fn test_empty_db_stats() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
    ]);
    assert!(status.success());
    assert_eq!(v["total_notes"], 0);
}

#[test]
fn test_empty_db_check() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], true);
}

#[test]
fn test_empty_db_resolve() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "resolve",
        "--title",
        "nothing",
    ]);
    assert!(status.success());
    assert_eq!(v["total"], 0);
}

// ----------------------------------------------------------------
// ERROR PATH: missing --db produces structured JSON error
// ----------------------------------------------------------------
#[test]
fn test_missing_db_json_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(pkms_binary())
        .args(["--output-format", "json", "stats"])
        .env("XDG_CONFIG_HOME", dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| panic!("Expected JSON error, got: {}", stdout));
    assert!(v.get("error").is_some());
}

// ----------------------------------------------------------------
// PARAMETERIZED: every command with --output-format json produces valid JSON
// ----------------------------------------------------------------
#[test]
fn test_all_commands_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<(Vec<String>, bool)> = vec![
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "check".into(),
            ],
            false,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "stats".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "orphans".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "check".into(),
                "--file-links".into(),
                "--attachment-links".into(),
                "--id-links".into(),
            ],
            false,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "stats".into(),
                "--hubs".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "stats".into(),
                "--tags".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "info".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "resolve".into(),
                "--title".into(),
                "Note".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "query".into(),
                "Note".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "path".into(),
                "Note A".into(),
                "Note C".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "context".into(),
                "Note A".into(),
                "--depth".into(),
                "1".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "validate".into(),
                "Note A".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "get".into(),
                "Note A".into(),
                "--links".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "suggest".into(),
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "new".into(),
                "Parametric Test".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "fix".into(),
                "ffffffff-ffff-4fff-ffff-ffffffffffff".into(),
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "agenda".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "todo".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "stats".into(),
                "--todos".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--output-format".into(),
                "json".into(),
                "check".into(),
                "--agenda".into(),
            ],
            false,
        ),
    ];
    for (args, expect_success) in &cases {
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = Command::new(pkms_binary())
            .args(&args_refs)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if *expect_success {
            assert!(
                output.status.success(),
                "Expected success for {:?}\nstdout: {}\nstderr: {}",
                args_refs,
                stdout,
                stderr
            );
        }
        let trimmed = stdout.trim();
        assert!(
            trimmed.starts_with('{'),
            "Expected JSON object for {:?}\nstdout: {}\nstderr: {}",
            args_refs,
            stdout,
            stderr
        );
        let v: serde_json::Value = serde_json::from_str(trimmed)
            .unwrap_or_else(|_| panic!("Invalid JSON for {:?}: {}", args_refs, stdout));
        assert!(v.is_object(), "Expected object for {:?}", args_refs);
    }
}

// ----------------------------------------------------------------
// --output-format json exit code 1 for business-logic failure
// ----------------------------------------------------------------
#[test]
fn test_json_error_exit_code() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "validate".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "get".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "suggest".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--output-format".into(),
            "json".into(),
            "context".into(),
            "DoesNotExist".into(),
        ],
    ];
    for args in &cases {
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, _stderr, status) = run(&args_refs);
        assert!(!status.success(), "Expected failure for {:?}", args_refs);
        let trimmed = stdout.trim();
        assert!(
            trimmed.starts_with('{'),
            "Expected JSON for {:?}",
            args_refs
        );
        let v: serde_json::Value = serde_json::from_str(trimmed).unwrap_or_else(|_| {
            panic!("Invalid JSON on error path for {:?}: {}", args_refs, stdout)
        });
        assert!(
            v.get("error").is_some(),
            "Expected error key for {:?}",
            args_refs
        );
    }
}

// ----------------------------------------------------------------
// SNAPSHOT TESTS: human output golden files
// ----------------------------------------------------------------
fn normalize_snapshot(output: &str, root: &std::path::Path) -> String {
    output.replace(root.to_str().unwrap(), "<DB_ROOT>")
}

#[test]
fn test_snapshot_path() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) =
        run(&["--db", root.to_str().unwrap(), "path", "Note A", "Note C"]);
    insta::assert_snapshot!("path_human", normalize_snapshot(&stdout, &root));
}

#[test]
fn test_snapshot_resolve() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) =
        run(&["--db", root.to_str().unwrap(), "resolve", "--title", "Note"]);
    insta::assert_snapshot!("resolve_query_human", normalize_snapshot(&stdout, &root));
}

// ----------------------------------------------------------------
// PIPE TESTS
// ----------------------------------------------------------------
fn run_pipe_ndjson(producer_args: &[&str], consumer_args: &[&str]) -> (String, ExitStatus) {
    let (stdout, _stderr, status) = run_pipe(producer_args, consumer_args);
    (stdout, status)
}

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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("uuid").is_some(),
            "expected suggest output with uuid, got: {}",
            line
        );
        assert!(
            v.get("score").is_some(),
            "expected suggest output with score, got: {}",
            line
        );
    }
}

// ----------------------------------------------------------------
// AGENDA
// ----------------------------------------------------------------
#[test]
fn test_agenda_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda"]);
    assert!(status.success(), "agenda failed: {stdout}");
    assert!(stdout.contains("planned item"), "stdout: {stdout}");
}

#[test]
fn test_agenda_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    assert!(
        v["items"].as_array().unwrap().len() >= 1,
        "expected at least 1 agenda item"
    );
}

#[test]
fn test_agenda_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "agenda",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
        assert!(v.get("todo_state").is_some());
    }
}

#[test]
fn test_agenda_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--include",
        "TODO",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let todo_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("TODO"))
        .collect();
    assert!(
        todo_items.len() >= 1,
        "expected TODO items with --include TODO"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "TODO", "expected all items to be TODO");
    }
}

#[test]
fn test_agenda_missing_agenda() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--missing-agenda",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty(), "expected items missing agenda");
    for item in items {
        assert_eq!(item["has_agenda_tag"], false);
    }
}

#[test]
fn test_agenda_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "agenda",
        "--exclude",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.is_empty(),
        "expected no DONE items with --exclude DONE"
    );
}

#[test]
fn test_agenda_today_and_week() {
    let (_dir, root) = setup_db();
    let (_, _stderr, status) = run(&["--db", root.to_str().unwrap(), "agenda", "--today"]);
    assert!(status.success());

    let (_, _stderr2, status2) = run(&["--db", root.to_str().unwrap(), "agenda", "--week"]);
    assert!(status2.success());
}

// ----------------------------------------------------------------
// TODO
// ----------------------------------------------------------------
#[test]
fn test_todo_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "todo"]);
    assert!(status.success(), "todo failed: {stdout}");
    assert!(stdout.contains("TODO"), "stdout: {stdout}");
}

#[test]
fn test_todo_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
    ]);
    assert!(status.success());
    assert!(v.get("total").is_some(), "expected total field");
    assert!(v.get("items").is_some(), "expected items field");
    assert!(
        v["items"].as_array().unwrap().len() >= 1,
        "expected at least 1 todo item"
    );
}

#[test]
fn test_todo_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "todo",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
        assert!(v.get("todo_state").is_some());
    }
}

#[test]
fn test_todo_include() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--include",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.len() >= 1,
        "expected DONE items with --include DONE"
    );
    for item in items {
        let state = item["todo_state"].as_str().unwrap_or("");
        assert_eq!(state, "DONE", "expected all items to be DONE");
    }
}

#[test]
fn test_todo_exclude() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--exclude",
        "DONE",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    let done_items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|i| i["todo_state"].as_str() == Some("DONE"))
        .collect();
    assert!(
        done_items.is_empty(),
        "expected no DONE items with --exclude DONE"
    );
}

#[test]
fn test_todo_missing_agenda() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "todo",
        "--missing-agenda",
    ]);
    assert!(status.success());
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty(), "expected items missing agenda");
    for item in items {
        assert_eq!(item["has_agenda_tag"], false);
    }
}

#[test]
fn test_todo_sort_state() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "todo", "--sort", "state"]);
    assert!(status.success(), "todo --sort state failed: {stdout}");
    assert!(
        stdout.contains("TODO") || stdout.contains("DONE"),
        "stdout: {stdout}"
    );
}

// ----------------------------------------------------------------
// STATS --TODOS
// ----------------------------------------------------------------
#[test]
fn test_stats_todos_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "stats", "--todos"]);
    assert!(status.success());
    assert!(
        stdout.contains("TODO Statistics") || stdout.contains("Total TODO headings"),
        "stdout: {stdout}"
    );
}

#[test]
fn test_stats_todos_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "stats",
        "--todos",
    ]);
    assert!(status.success());
    assert!(v.get("total_todo_headings").is_some());
    assert!(v.get("files_with_todos").is_some());
    assert!(v.get("by_state").is_some());
}

// ----------------------------------------------------------------
// CHECK --AGENDA
// ----------------------------------------------------------------
#[test]
fn test_check_agenda() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "check", "--agenda"]);
    assert!(
        !status.success(),
        "check --agenda should find issues: {stdout}"
    );
    assert!(
        stdout.contains("Missing :agenda: tag") || stdout.contains("agenda"),
        "stdout: {stdout}"
    );
}

#[test]
fn test_check_agenda_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--agenda",
    ]);
    assert!(!status.success());
    assert!(
        v.get("agenda_issues").is_some(),
        "expected agenda_issues field"
    );
    let issues = v["agenda_issues"].as_array().unwrap();
    assert!(
        issues.len() >= 1,
        "expected at least 1 agenda issue, got {}",
        issues.len()
    );
    assert!(issues[0]["uuid"].is_string());
    assert!(issues[0]["todo_count"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn test_check_agenda_healthy_false() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--agenda",
    ]);
    assert!(!status.success());
    assert_eq!(v["healthy"], false);
}

// ----------------------------------------------------------------
// VALIDATE with agenda check
// ----------------------------------------------------------------
#[test]
fn test_validate_agenda_issue() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Missing Agenda Tag",
    ]);
    assert!(status.success());
    let issues = v["issues"].as_array().unwrap();
    let has_agenda_issue = issues
        .iter()
        .any(|i| i.as_str().map_or(false, |s| s.contains("agenda")));
    assert!(
        has_agenda_issue,
        "validate should report missing :agenda: filetag, issues: {:?}",
        issues
    );
    assert_eq!(v["healthy"], false);
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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("uuid").is_some(),
            "expected validate output, got: {}",
            line
        );
        assert!(
            v.get("healthy").is_some(),
            "expected healthy field, got: {}",
            line
        );
    }
}

// ----------------------------------------------------------------
// HEADING UUID TESTS
// ----------------------------------------------------------------
#[test]
fn test_new_with_heading() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("test-heading-note.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
#+title: Test Heading Note

* My Heading
Some content
"#,
    )
    .unwrap();

    let (stdout, _stderr, status) = run(&[
        "--db",
        db,
        "new",
        "Test Heading Note",
        "--create",
        "--heading",
        "My Heading",
    ]);
    assert!(status.success(), "stdout: {}", stdout);
    assert!(stdout.contains("Heading UUID"));

    let content = std::fs::read_to_string(&note_path).unwrap();
    let id_count = content.matches(":ID:").count();
    assert_eq!(id_count, 2, "should have note-level and heading-level IDs");
}

#[test]
fn test_new_with_heading_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("json-heading-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
#+title: JSON Heading Test

* JSON Section
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "new",
        "JSON Heading Test",
        "--create",
        "--heading",
        "JSON Section",
    ]);
    assert!(status.success());
    assert!(v.get("heading").is_some());
    assert_eq!(v["heading"]["title"], "JSON Section");
    assert!(v["heading"]["uuid"].is_string());
}

#[test]
fn test_resolve_heading_uuid() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    let note_path = note_dir.join("resolve-heading-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       33333333-3333-4333-8333-333333333333
:END:
#+title: Resolve Heading Test

* My Section
:PROPERTIES:
:ID:       44444444-4444-4444-8444-444444444444
:END:
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "resolve",
        "--uuid",
        "44444444-4444-4444-8444-444444444444",
    ]);
    assert!(status.success(), "resolve failed: {:?}", v);
    assert_eq!(v["total"], 1, "should find 1 note by heading UUID");
    assert_eq!(v["results"][0]["title"], "Resolve Heading Test");
}

#[test]
fn test_get_headings_with_uuids() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-heading-uuid-test.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       55555555-5555-4555-8555-555555555555
:END:
#+title: Get Heading UUIDs Test

* Section One
:PROPERTIES:
:ID:       66666666-6666-4666-8666-666666666666
:END:
Text
* Section Two
Some content
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "get",
        "Get Heading UUIDs Test",
        "--headings",
    ]);
    assert!(status.success());
    assert!(v["node"]["headings"].is_array());
    let headings = v["node"]["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2);
    assert_eq!(
        headings[0]["uuid"], "66666666-6666-4666-8666-666666666666",
        "first heading should have uuid"
    );
    assert!(
        headings[1].get("uuid").is_none(),
        "second heading should not have uuid"
    );
}

#[test]
fn test_link_to_heading_resolves() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    fs::create_dir_all(&note_dir).unwrap();

    fs::write(
        note_dir.join("heading-target.org"),
        r#":PROPERTIES:
:ID:       target-heading-note-uuid
:END:
#+title: Heading Target

* Target Section
:PROPERTIES:
:ID:       heading-target-uuid-aaaa
:END:
"#,
    )
    .unwrap();

    fs::write(
        note_dir.join("heading-target.org"),
        r#":PROPERTIES:
:ID:       abababab-abab-4aba-8aba-abababababab
:END:
#+title: Heading Target

* Target Section
:PROPERTIES:
:ID:       bcbcbcbc-bcbc-4bbc-8bbc-bcbcbcbcbcbc
:END:
"#,
    )
    .unwrap();

    fs::write(
        note_dir.join("heading-linker.org"),
        r#":PROPERTIES:
:ID:       cdcdcdcd-cdcd-4cdc-8cdc-cdcdcdcdcdcd
:END:
#+title: Heading Linker

[[id:bcbcbcbc-bcbc-4bbc-8bbc-bcbcbcbcbcbc][Link to heading]]
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "validate",
        "Heading Linker",
    ]);
    assert!(status.success());
    assert_eq!(v["healthy"], true);
    assert!(
        v["broken_internal"].as_array().unwrap().is_empty(),
        "link to heading UUID should not be broken"
    );
}

#[test]
fn test_heading_uuid_duplicate_detection() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("common");
    fs::create_dir_all(&note_dir).unwrap();

    fs::write(
        note_dir.join("dup-heading-1.org"),
        r#":PROPERTIES:
:ID:       dededede-dede-4ded-8ded-dededededede
:END:
#+title: Dup Heading 1

* Section
:PROPERTIES:
:ID:       efefefef-efef-4efe-8efe-efefefefefef
:END:
"#,
    )
    .unwrap();

    fs::write(
        note_dir.join("dup-heading-2.org"),
        r#":PROPERTIES:
:ID:       fafafafa-fafa-4faf-8faf-fafafafafafa
:END:
#+title: Dup Heading 2

* Section
:PROPERTIES:
:ID:       efefefef-efef-4efe-8efe-efefefefefef
:END:
"#,
    )
    .unwrap();

    let (v, status) = run_json(&["--db", db, "--output-format", "json", "check", "--id-links"]);
    assert!(!status.success(), "check should report issues");
    let dups = v["duplicates"]["duplicate_uuids"].as_array().unwrap();
    let clash = dups
        .iter()
        .find(|d| d["value"] == "efefefef-efef-4efe-8efe-efefefefefef");
    assert!(
        clash.is_some(),
        "should report clashing heading UUID, got dups: {:?}",
        dups
    );
}

#[test]
fn test_validate_heading_uuids_field() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("validate-heading-uuids.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       bebebebe-bebe-4beb-8beb-bebebebebebe
:END:
#+title: Validate Heading UUIDs

* Section A
:PROPERTIES:
:ID:       cacacaca-caca-4cac-8cac-cacacacacaca
:END:
Text
"#,
    )
    .unwrap();

    let (v, status) = run_json(&[
        "--db",
        db,
        "--output-format",
        "json",
        "validate",
        "Validate Heading UUIDs",
    ]);
    assert!(status.success());
    assert!(v.get("heading_uuids").is_some());
    let heading_uuids = v["heading_uuids"].as_array().unwrap();
    assert_eq!(heading_uuids.len(), 1);
    assert_eq!(heading_uuids[0], "cacacaca-caca-4cac-8cac-cacacacacaca");
}

#[test]
fn test_get_headings_text_with_uuids() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap();

    let note_dir = root.join("roam").join("personal");
    let note_path = note_dir.join("get-text-heading-uuid.org");
    fs::write(
        &note_path,
        r#":PROPERTIES:
:ID:       f0f0f0f0-f0f0-4f0f-8f0f-f0f0f0f0f0f0
:END:
#+title: Get Text Heading UUID

* Visible Heading
:PROPERTIES:
:ID:       f1f1f1f1-f1f1-4f1f-8f1f-f1f1f1f1f1f1
:END:
"#,
    )
    .unwrap();

    let (stdout, _stderr, status) = run(&[
        "--db",
        db,
        "get",
        "Get Text Heading UUID",
        "--headings",
        "--no-content",
    ]);
    assert!(status.success());
    assert!(stdout.contains("f1f1f1f1-f1f1-4f1f-8f1f-f1f1f1f1f1f1"));
    assert!(stdout.contains("Visible Heading"));
}

// ----------------------------------------------------------------
// DUPLICATE UUID SCENARIOS (systematic coverage)
// ----------------------------------------------------------------
// Scenarios tested:
//   #2 Pₐ = Hₐ   heading equals own note's primary
//   #3 Hₐ₁ = Hₐ₂ two headings in same note share UUID
//   #4 Pₐ = Hᵦ   primary of A equals heading of B
//   #5 Hₐ = Hᵦ   heading in A equals heading in B

fn setup_clean_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("db");
    fs::create_dir_all(root.join("roam")).unwrap();
    (dir, root)
}

fn db_write(root: &std::path::Path, name: &str, content: &str) {
    fs::write(root.join("roam").join(name), content).unwrap();
}

// --- check tests ---

#[test]
fn test_check_heading_equals_primary() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"),
        "scenario #2: heading equals own primary, got: {:?}",
        dups
    );
}

#[test]
fn test_check_heading_heading_same_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading One
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Heading Two
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #3: two headings share UUID, got: {:?}",
        dups
    );
}

#[test]
fn test_check_primary_equals_heading_other_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #4: primary of B equals heading of A, got: {:?}",
        dups
    );
}

#[test]
fn test_check_heading_heading_cross_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note B

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "check",
        "--id-links",
    ]);
    let dups: Vec<&str> = v["duplicates"]["duplicate_uuids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert!(
        dups.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"),
        "scenario #5: heading in A equals heading in B, got: {:?}",
        dups
    );
}

// --- validate tests ---

#[test]
fn test_validate_heading_equals_primary() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #2: validate should detect heading equals primary"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("Duplicate UUID")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_heading_heading_same_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Heading One
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Heading Two
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #3: validate should detect two headings sharing UUID"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues
            .iter()
            .any(|i| i.contains("used by multiple headings")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_primary_equals_heading_other_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Note B
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note A",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #4: validate should detect heading matching another note's primary"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("belongs to another note")),
        "validate issues: {:?}",
        issues
    );
}

#[test]
fn test_validate_heading_heading_cross_file() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note B

* Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
"#,
    );
    // Validate Note B (the second note indexed), whose heading clashes with Note A's heading
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "validate",
        "Note B",
    ]);
    assert_eq!(
        v["healthy"], false,
        "scenario #5: validate should detect heading matching another note's heading"
    );
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    assert!(
        issues.iter().any(|i| i.contains("belongs to another note")),
        "validate issues: {:?}",
        issues
    );
}

// ----------------------------------------------------------------
// PART 6: Heading-level backlinks and suggest for headings
// ----------------------------------------------------------------
#[test]
fn test_suggest_with_heading_uuid() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note

* Special Topic
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
Content about special topic
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    assert!(
        v["suggestions"].as_array().unwrap().is_empty(),
        "no other notes to suggest, but command should succeed"
    );
    assert_eq!(v["target"], "Note");
}

#[test]
fn test_suggest_with_heading_uuid_has_context() {
    let (_dir, root) = setup_clean_db();
    db_write(
        &root,
        "note_a.org",
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

* Special Section
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
Content about special topic
[[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][ref]]
"#,
    );
    db_write(
        &root,
        "note_b.org",
        r#":PROPERTIES:
:ID:       cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
#+title: Note B

Some content
"#,
    );
    let (v, _status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "json",
        "suggest",
        "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    ]);
    let suggestions = v["suggestions"].as_array().unwrap();
    assert!(!suggestions.is_empty(), "should have suggestions");
    // The heading context should mention the heading title
    let has_context = suggestions
        .iter()
        .any(|s| s.get("heading_context").and_then(|c| c.as_str()).is_some());
    assert!(
        has_context,
        "suggest should include heading_context when targeting a heading UUID"
    );
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
    assert!(!stdout.is_empty(), "expected output from pipe");
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Bad JSON line '{}': {}", line, e));
        assert!(
            v.get("node").is_some(),
            "expected get output, got: {}",
            line
        );
    }
}
