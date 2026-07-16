use super::*;
use anyhow::Result;
use pkms_org::OrgConfig;
use pkms_org::graph::Graph;
use std::fs;
use std::path::PathBuf;

fn web_config(db_root: PathBuf) -> WebConfig {
    WebConfig {
        org: OrgConfig {
            ignore_patterns: Vec::new(),
            db_root,
            home_dir: None,
        },
        open_todo_states: vec!["TODO".to_string()],
        closed_todo_states: vec!["DONE".to_string()],
    }
}

fn noop_open_target(
    _graph: &Graph,
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

#[test]
fn renders_all_cases_preserved_from_entrance_note() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let roam = root.join("roam");
    fs::create_dir_all(&roam).unwrap();
    fs::write(
        roam.join("entrance.org"),
        include_str!("testdata/entrance-rendering-cases.org"),
    )
    .unwrap();
    fs::write(
        roam.join("target.org"),
        r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Linked note

* Linked heading
:PROPERTIES:
:ID:       dddddddd-dddd-4ddd-8ddd-dddddddddddd
:END:
Heading body.
"#,
    )
    .unwrap();
    let mut config = web_config(root);
    config.org.home_dir = Some(dir.path().to_path_buf());
    config.open_todo_states = vec!["TODO".into(), "WAITING".into(), "PROBLEM".into()];
    config.closed_todo_states = vec!["DONE".into(), "CANCELED".into()];
    let viewer = NoteViewer::new(config, noop_open_target, "true").unwrap();

    let page = viewer
        .respond(ViewerRequest {
            method: ViewerMethod::Get,
            path: "/".to_string(),
            query: Some("id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()),
        })
        .unwrap();
    let html = String::from_utf8(page.body).unwrap();

    assert_eq!(page.status, 200);
    assert!(html.contains("<h1>Entrance rendering cases</h1>"));
    assert!(html.contains("#agenda"));
    assert!(html.contains("#projects"));
    assert!(html.contains("class=\"katex\""));
    assert!(html.contains("S=\\pi{r}^2"));

    assert!(html.contains("kind=file&amp;target=~%2Fwork%2Fproject%2Fsource.rs%3A%3A40"));
    assert!(
        html.contains(
            "kind=file&amp;target=~%2Fwork%2Fproject%2Fsource.rs%3A%3Apub%20const%20VALUE"
        )
    );
    assert!(html.contains("line number</a>"));
    assert!(html.contains("line number but without"));
    assert!(html.contains("File to analyze: header.h."));
    assert!(!html.contains("[[header.h]]"));
    assert!(html.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
    assert!(html.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb#h-6\""));
    assert!(html.contains("data-preview-id=\"dddddddd-dddd-4ddd-8ddd-dddddddddddd\""));
    assert!(html.contains("href=\"https://example.org/docs\" rel=\"noreferrer\""));
    assert!(html.contains("href=\"https://example.com/path?x=1\" rel=\"noreferrer\""));

    assert!(html.contains("<figcaption>Source: rust</figcaption>"));
    assert!(html.contains("<figcaption>Source: c++</figcaption>"));
    assert!(html.contains("<figcaption>Source: sh</figcaption>"));
    assert!(html.contains("<figcaption>Source</figcaption>"));
    assert!(html.contains("<code class=\"syn-code\">"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<th scope=\"col\">head 1</th>"));
    assert!(html.contains("<td><a href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));

    for block_class in [
        "org-block-export",
        "org-block-center",
        "org-block-comment",
        "org-block-example",
        "org-block-quote",
        "org-block-verse",
    ] {
        assert!(
            html.contains(block_class),
            "missing block class {block_class}"
        );
    }
    assert!(html.contains("<div class=\"org-block-caption\">ascii caption</div>"));
    assert!(html.contains("<div class=\"org-block-caption\">center caption</div>"));
    assert!(html.contains("<div class=\"org-block-caption\">generic export caption</div>"));
    assert!(html.contains("<div class=\"org-block-caption\">latex caption</div>"));
    assert!(html.contains("<div class=\"org-block-caption\">source caption</div>"));
    assert!(html.contains("&lt;strong&gt;html export is escaped&lt;/strong&gt;"));
    assert!(html.contains("class=\"katex-display\""));
    assert!(html.contains("<em>block</em>"));
    assert!(html.contains("example &lt;block&gt;"));

    assert!(html.contains("<code class=\"inline-code code-orange\">starting</code>"));
    assert!(html.contains("<code class=\"inline-code\">think</code>"));
    assert!(html.contains("<code class=\"inline-code code-mention\">@OneName</code>"));
    assert!(html.contains("<code class=\"inline-code code-mention\">@another_name</code>"));
    assert!(html.contains("<li>first level<ul>"));
    assert!(html.contains("<li>second level<ul>"));
    assert!(html.contains("<li>third level<ol>"));
    assert!(html.contains("<ol type=\"a\">"));
    assert!(html.contains("First wrapped line continued first line"));
    assert!(html.contains(
        "<li>Long unordered item starts here and continues on its second source line then on its third source line and finally on its fourth source line</li>\n<li>Following unordered sibling</li>"
    ));
    assert!(html.contains("Alpha child first line continued alpha child"));
    assert!(html.contains("Number child first line continued number child"));
    assert!(html.contains(
        "<li>Long numbered item starts here and continues on its second source line then on its third source line and finally on its fourth source line</li>\n<li>Following numbered sibling</li>"
    ));
    assert!(html.contains("<li class=\"checked-item\">[X] Complete</li>"));
    assert!(html.contains("<li>[-] In progress</li>"));
    assert!(html.contains("<li>[ ] Open<ul>"));

    for state in ["TODO", "DONE", "WAITING", "CANCELED", "PROBLEM"] {
        assert!(
            html.contains(&format!("<span class=\"todo\">{state}</span>")),
            "missing TODO badge for {state}"
        );
    }
    assert!(html.contains("<span class=\"todo\">DONE</span> [#B] Closed task</a>"));
    assert!(html.contains(
        "<span class=\"todo\">DONE</span> [#B] Closed task <span class=\"heading-tags\""
    ));
    assert!(html.contains("<span class=\"todo\">CANCELED</span> [#C] Canceled task</h3>"));
    assert!(html.contains("class=\"heading-tags\""));
    assert!(html.contains("class=\"planning planning-scheduled\""));
    assert!(html.contains("class=\"planning planning-deadline\""));
    assert!(!html.contains("<p>SCHEDULED:"));
    assert!(!html.contains("<p>DEADLINE:"));
}
