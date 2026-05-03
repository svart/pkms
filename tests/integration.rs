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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "check"]);
    assert!(!status.success());
    assert!(v.get("stats").is_some());
    assert_eq!(v["healthy"], false);
    assert!(v.get("broken_links").is_some());
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
        "--json",
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
        "--json",
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "stats"]);
    assert!(status.success());
    assert!(v.get("total_notes").is_some());
    assert!(v.get("hubs").is_some());
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "orphans"]);
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
// BROKEN
// ----------------------------------------------------------------
#[test]
fn test_broken_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "broken"]);
    assert!(status.success());
    assert!(stdout.contains("Broken Note"));
}

#[test]
fn test_broken_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "broken"]);
    assert!(status.success());
    assert!(v["count"].as_u64().unwrap_or(0) >= 1);
    assert!(v["links"][0]["source_uuid"].is_string());
}

#[test]
fn test_broken_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "broken",
    ]);
    assert!(status.success());
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 1);
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("source_uuid").is_some());
    }
}

// ----------------------------------------------------------------
// HUBS
// ----------------------------------------------------------------
#[test]
fn test_hubs_human() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "hubs"]);
    assert!(status.success());
}

#[test]
fn test_hubs_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "hubs"]);
    assert!(status.success());
    assert!(v["hubs"].as_array().map_or(false, |h| !h.is_empty()));
    assert!(v["hubs"][0]["uuid"].is_string());
}

#[test]
fn test_hubs_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "hubs",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
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
        "--json",
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
        "--json",
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

#[test]
fn test_context_include_outgoing_false() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "context",
        "Note A",
        "--depth",
        "1",
        "--include-outgoing",
        "false",
    ]);
    assert!(status.success());
    let ctx = v["context"].as_str().unwrap_or("");
    // Should still have the note content but NOT forward links
    assert!(ctx.contains("Note A"), "should contain target title");
    assert!(
        !ctx.contains("→"),
        "should not contain forward link arrows: {}",
        ctx
    );
    // "Note B" appears in the raw file content as [[id:...][Note B]], so check for arrow prefix
    assert!(
        !ctx.contains("\n  → Note B"),
        "should not contain neighbor as forward link"
    );
}

#[test]
fn test_context_include_incoming_false() {
    // Note B has an incoming link from Note A; with --include-incoming=false
    // the backlinks section should still appear since we use depth
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "context",
        "Note B",
        "--depth",
        "1",
        "--include-incoming",
        "false",
    ]);
    assert!(status.success());
    let ctx = v["context"].as_str().unwrap_or("");
    // Should still show forward links (Note C) but NOT backlinks (Note A)
    assert!(ctx.contains("Note C"), "should contain forward linked note");
    assert!(!ctx.contains("←"), "should not contain backlink arrows");
}

#[test]
fn test_context_template_custom() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "context",
        "Note A",
        "--depth",
        "1",
        "--template",
        "Title: {{title}}\nContent:\n{{content}}",
    ]);
    assert!(status.success());
    let ctx = v["context"].as_str().unwrap_or("");
    assert!(ctx.starts_with("Title: Note A"), "ctx: {}", ctx);
    assert!(ctx.contains("Content:"));
    assert!(
        !ctx.contains("UUID:"),
        "should not contain UUID from default template"
    );
}

#[test]
fn test_context_template_conditional() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "context",
        "Note A",
        "--depth",
        "0",
        "--template",
        "{{#tags}}has tags{{/tags}}{{#aliases}}has aliases{{/aliases}}",
    ]);
    assert!(status.success());
    let ctx = v["context"].as_str().unwrap_or("");
    // Note A has no tags or aliases, so both conditionals should be empty
    assert_eq!(
        ctx, "",
        "expected empty output for missing conditionals, got: {}",
        ctx
    );
}

// ----------------------------------------------------------------
// RESOLVE
// ----------------------------------------------------------------
#[test]
fn test_resolve_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "resolve"]);
    assert!(status.success());
    assert!(stdout.contains("Note A") || stdout.contains("Total:"));
}

#[test]
fn test_resolve_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "resolve"]);
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "resolve", "Note"]);
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
        "Note A",
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
        "--json",
        "fix",
        "ffffffff-ffff-4fff-ffff-ffffffffffff",
        "Note A",
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
        "Note A",
    ]);
    // fix succeeds (0 replacements found); check output mentions 0
    assert!(status.success());
    assert!(stdout.contains("0 broken link") || stdout.contains("Would fix"));
}

// ----------------------------------------------------------------
// SUGGEST
// ----------------------------------------------------------------
#[test]
fn test_suggest_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "suggest", "Note A"]);
    assert!(status.success());
    assert!(stdout.contains("Suggestions") || stdout.contains("Note B"));
}

#[test]
fn test_suggest_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "suggest",
        "Note A",
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "new", "Test Note"]);
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
        "--json",
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
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "get",
        "Note A",
        "--depth",
        "1",
    ]);
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
        "--json",
        "get",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success());
    assert_eq!(v["node"]["title"], "Note A");
    assert!(v.get("neighbors").is_some());
}

#[test]
fn test_get_out() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "get", "Note A", "--out"]);
    assert!(status.success());
    assert!(stdout.contains("--- Content ---"));
}

#[test]
fn test_get_graph() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "get", "Note A", "--graph"]);
    assert!(status.success());
    assert!(
        stdout.contains("\u{2514}") || stdout.contains("\u{2502}") || stdout.contains("\u{25cf}")
    );
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "query", "Note"]);
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
fn test_query_tag_filter() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "query",
        "Note",
        "--tag",
        "learning",
    ]);
    assert!(status.success());
}

#[test]
fn test_query_limit() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "query",
        "Note",
        "--limit",
        "1",
    ]);
    assert!(status.success());
    assert_eq!(v["total_results"], 1);
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "info"]);
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
        "--json",
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
// SUBGRAPH
// ----------------------------------------------------------------
#[test]
fn test_subgraph_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "subgraph",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success());
    assert!(stdout.contains("Note A") || stdout.contains("Vertices:"));
}

#[test]
fn test_subgraph_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "subgraph",
        "Note A",
        "--depth",
        "1",
    ]);
    assert!(status.success());
    assert!(v.get("root_uuid").is_some());
    assert!(v.get("vertex_count").is_some());
    assert!(v.get("nodes").is_some());
}

#[test]
fn test_subgraph_note_not_found() {
    let (_dir, root) = setup_db();
    let (_stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "subgraph", "Nonexistent"]);
    assert!(!status.success());
}

// ----------------------------------------------------------------
// TAGS
// ----------------------------------------------------------------
#[test]
fn test_tags_human() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "tags"]);
    assert!(status.success());
    assert!(stdout.contains("learning") || stdout.contains("emacs"));
}

#[test]
fn test_tags_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "tags"]);
    assert!(status.success());
    assert!(v["tags"].as_array().map_or(false, |t| !t.is_empty()));
    assert!(v["tags"][0]["tag"].is_string());
}

#[test]
fn test_tags_ndjson() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "tags",
    ]);
    assert!(status.success());
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("tag").is_some());
    }
}

#[test]
fn test_tags_tag_filter() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "tags", "--tag", "learning"]);
    assert!(status.success());
    assert!(stdout.contains("Tag: learning") || stdout.contains("Tagged Note"));
}

#[test]
fn test_tags_tag_filter_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "tags",
        "--tag",
        "learning",
    ]);
    assert!(status.success());
    assert_eq!(v["tags"][0]["tag"], "learning");
    assert!(
        v["tags"][0]["notes"]
            .as_array()
            .map_or(false, |n| !n.is_empty())
    );
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
        "--json",
        "check",
        "--file-links",
    ]);
    assert!(!status.success());
    assert!(v.get("broken_file_links").is_some());
    let broken_files = v["broken_file_links"].as_array().unwrap();
    assert!(broken_files.len() >= 1, "expected broken file links");
    assert!(broken_files[0]["source_title"].is_string());
    assert!(broken_files[0]["target_path"].is_string());
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
        "--json",
        "check",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(v.get("broken_attachment_links").is_some());
    let broken_attach = v["broken_attachment_links"].as_array().unwrap();
    assert!(broken_attach.len() >= 1, "expected broken attachment links");
    assert!(broken_attach[0]["source_title"].is_string());
    assert!(broken_attach[0]["target_path"].is_string());
}

#[test]
fn test_check_file_and_attachment_links_json() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "check",
        "--file-links",
        "--attachment-links",
    ]);
    assert!(!status.success());
    assert!(v.get("broken_file_links").is_some());
    assert!(v.get("broken_attachment_links").is_some());
    assert!(v["broken_file_links"].as_array().unwrap().len() >= 1);
    assert!(v["broken_attachment_links"].as_array().unwrap().len() >= 1);
}

// ----------------------------------------------------------------
// ERROR PATH: note not found (with --json, expect structured error)
// ----------------------------------------------------------------
#[test]
fn test_error_note_not_found_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "validate".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "get".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "suggest".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "subgraph".into(),
            "Nonexistent".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
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
        "--json",
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
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "stats"]);
    assert!(status.success());
    assert_eq!(v["total_notes"], 0);
}

#[test]
fn test_empty_db_check() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "check"]);
    assert!(status.success());
    assert_eq!(v["healthy"], true);
}

#[test]
fn test_empty_db_resolve() {
    let (_dir, root) = setup_empty_db();
    let (v, status) = run_json(&["--db", root.to_str().unwrap(), "--json", "resolve"]);
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
        .args(["--json", "stats"])
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
// --output-format ndjson with --quiet
// ----------------------------------------------------------------
#[test]
fn test_ndjson_quiet() {
    let (_dir, root) = setup_db();
    let (stdout, stderr, status) = run(&[
        "--db",
        root.to_str().unwrap(),
        "--output-format",
        "ndjson",
        "--quiet",
        "resolve",
    ]);
    assert!(status.success());
    assert!(
        stderr.is_empty(),
        "Expected empty stderr with --quiet, got: {}",
        stderr
    );
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("uuid").is_some());
    }
}

// ----------------------------------------------------------------
// PARAMETERIZED: every command with --json produces valid JSON
// ----------------------------------------------------------------
#[test]
fn test_all_commands_json() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<(Vec<String>, bool)> = vec![
        (
            vec!["--db".into(), db.clone(), "--json".into(), "check".into()],
            false,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "stats".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "orphans".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "broken".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "hubs".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "tags".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "info".into()],
            true,
        ),
        (
            vec!["--db".into(), db.clone(), "--json".into(), "resolve".into()],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--json".into(),
                "query".into(),
                "Note".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--json".into(),
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
                "--json".into(),
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
                "--json".into(),
                "validate".into(),
                "Note A".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--json".into(),
                "get".into(),
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
                "--json".into(),
                "suggest".into(),
                "Note A".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--json".into(),
                "new".into(),
                "Parametric Test".into(),
            ],
            true,
        ),
        (
            vec![
                "--db".into(),
                db.clone(),
                "--json".into(),
                "subgraph".into(),
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
                "--json".into(),
                "fix".into(),
                "ffffffff-ffff-4fff-ffff-ffffffffffff".into(),
                "Note A".into(),
            ],
            true,
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
// CLI AUTOMATION: --no-header
// ----------------------------------------------------------------
#[test]
fn test_no_header_orphans() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "orphans", "--no-header"]);
    assert!(status.success());
    // Should NOT contain "Orphan notes (N):" header
    assert!(
        !stdout.contains("Orphan notes"),
        "no-header should suppress header: {}",
        stdout
    );
    assert!(
        stdout.contains("Orphan Note"),
        "should still show items: {}",
        stdout
    );
}

#[test]
fn test_no_header_broken() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "broken", "--no-header"]);
    assert!(status.success());
    assert!(
        !stdout.contains("Broken links"),
        "no-header should suppress header: {}",
        stdout
    );
    assert!(
        stdout.contains("Broken Note"),
        "should still show items: {}",
        stdout
    );
}

#[test]
fn test_no_header_resolve() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) =
        run(&["--db", root.to_str().unwrap(), "resolve", "--no-header"]);
    assert!(status.success());
    assert!(
        !stdout.contains("Total:"),
        "no-header should suppress Total: {}",
        stdout
    );
    assert!(
        stdout.contains("Note A"),
        "should still show items: {}",
        stdout
    );
}

#[test]
fn test_count_only_broken() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, status) = run(&["--db", root.to_str().unwrap(), "broken", "--count"]);
    assert!(status.success());
    let count: usize = stdout
        .trim()
        .parse()
        .expect("--count should print just a number");
    assert!(count >= 1, "expected at least 1 broken link");
}

#[test]
fn test_count_only_orphans() {
    let (_dir, root) = setup_db();
    let (v, status) = run_json(&[
        "--db",
        root.to_str().unwrap(),
        "--json",
        "orphans",
        "--count",
    ]);
    assert!(status.success());
    assert!(v.get("count").is_some());
    assert!(v["count"].as_u64().unwrap_or(0) >= 1);
    // Should NOT have the full list
    assert!(v.get("orphans").is_none());
}

// ----------------------------------------------------------------
// --json exit code 1 for business-logic failure
// ----------------------------------------------------------------
#[test]
fn test_json_error_exit_code() {
    let (_dir, root) = setup_db();
    let db = root.to_str().unwrap().to_string();
    let cases: Vec<Vec<String>> = vec![
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "validate".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "get".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "suggest".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "context".into(),
            "DoesNotExist".into(),
        ],
        vec![
            "--db".into(),
            db.clone(),
            "--json".into(),
            "subgraph".into(),
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
fn test_snapshot_broken() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) = run(&["--db", root.to_str().unwrap(), "broken"]);
    insta::assert_snapshot!("broken_human", normalize_snapshot(&stdout, &root));
}

#[test]
fn test_snapshot_tags() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) = run(&["--db", root.to_str().unwrap(), "tags"]);
    insta::assert_snapshot!("tags_human", normalize_snapshot(&stdout, &root));
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
    let (stdout, _stderr, _status) = run(&["--db", root.to_str().unwrap(), "resolve", "Note"]);
    insta::assert_snapshot!("resolve_query_human", normalize_snapshot(&stdout, &root));
}
