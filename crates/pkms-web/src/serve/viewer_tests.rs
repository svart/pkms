use super::*;
use anyhow::Result;
use pkms_org::OrgConfig;
use pkms_org::graph::Graph;
use pkms_org::graph::tasks::TaskStateConfig;
use std::fs;
use std::path::PathBuf;

fn web_config(db_root: PathBuf) -> WebConfig {
    WebConfig {
        org: OrgConfig {
            new_notes_dir: Some(db_root.join("roam")),
            daily_notes_dir: Some(db_root.join("roam")),
            ignore_patterns: Vec::new(),
            db_root,
        },
        task_states: TaskStateConfig {
            valid_states: vec!["TODO".to_string(), "DONE".to_string()],
            open_states: vec!["TODO".to_string()],
            closed_states: vec!["DONE".to_string()],
        },
    }
}

fn noop_open_target(
    _graph: &Graph,
    _task_states: &TaskStateConfig,
    _target: &str,
    _editor: &str,
    _line: Option<usize>,
) -> Result<()> {
    Ok(())
}

#[test]
fn note_viewer_reuses_pkms_serve_routes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let roam = root.join("roam");
    fs::create_dir_all(&roam).unwrap();
    fs::write(
        roam.join("a.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Beta]]
"#,
    )
    .unwrap();
    fs::write(
        roam.join("b.org"),
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta

* Preview heading
Preview body.
"#,
    )
    .unwrap();
    let viewer = NoteViewer::new(web_config(root), noop_open_target, "true").unwrap();

    let page = viewer
        .respond(ViewerRequest {
            method: ViewerMethod::Get,
            path: "/".to_string(),
            query: Some("id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()),
        })
        .unwrap();
    let page_body = String::from_utf8(page.body).unwrap();

    assert_eq!(page.status, 200);
    assert_eq!(page.content_type, "text/html; charset=utf-8");
    assert!(page_body.contains("<h1>Alpha</h1>"));
    assert!(page_body.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
    assert!(page_body.contains("data-preview-url=\"/preview\""));

    let preview = viewer
        .respond(ViewerRequest {
            method: ViewerMethod::Get,
            path: "/preview".to_string(),
            query: Some("id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string()),
        })
        .unwrap();
    let preview_body = String::from_utf8(preview.body).unwrap();

    assert_eq!(preview.status, 200);
    assert!(preview_body.contains("<article class=\"note-body note-preview-body\">"));
    assert!(preview_body.contains("Preview body."));
    assert!(!preview_body.contains("<html"));
}
