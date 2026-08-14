use anyhow::{Context, Result};
use pkms_org::{Graph, OrgConfig};
use serde::Serialize;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

mod serve;

#[cfg(test)]
use serve::highlight::highlight_code;
use serve::http::ServeState;
#[cfg(test)]
use serve::http::{is_client_disconnect, open_response, resolve_file_target};
#[cfg(test)]
use serve::inline::{percent_decode, percent_encode, render_display_math, render_formatted_text};
use serve::{HttpResponse, http};
#[cfg(test)]
use serve::{assets, page_css, page_js, render_note_html, render_preview_html};

pub struct ServeOptions {
    pub target: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct WebConfig {
    pub org: OrgConfig,
    pub open_todo_states: Vec<String>,
    pub closed_todo_states: Vec<String>,
}

impl WebConfig {
    fn load_graph(&self) -> Result<Graph> {
        Graph::load_from(&self.org.scan_config(), &self.org.link_resolution_context())
    }

    #[cfg(test)]
    fn load_corpus(&self) -> Result<pkms_org::Corpus> {
        pkms_org::Corpus::load_from(&self.org.scan_config())
    }

    pub fn resolved_db_root(&self) -> &Path {
        &self.org.db_root
    }

    pub fn open_todo_states(&self) -> &[String] {
        &self.open_todo_states
    }

    pub fn closed_todo_states(&self) -> &[String] {
        &self.closed_todo_states
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServeStarted {
    pub url: String,
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub type OpenTargetFn = fn(&Graph, &str, &str, Option<usize>) -> Result<()>;

#[derive(Debug, Clone)]
pub(crate) enum InitialTarget {
    Org(pkms_org::domain::NoteId),
    File(FileTarget),
}

#[derive(Debug, Clone)]
pub(crate) struct FileTarget {
    pub(crate) path: PathBuf,
    pub(crate) request_path: String,
    pub(crate) format: FileFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileFormat {
    Org,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerMethod {
    Get,
    Head,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerRequest {
    pub method: ViewerMethod,
    pub path: String,
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl From<HttpResponse> for ViewerResponse {
    fn from(value: HttpResponse) -> Self {
        Self {
            status: value.status,
            content_type: value.content_type.as_str().to_string(),
            body: value.body,
        }
    }
}

pub struct NoteViewer {
    config: WebConfig,
    graph: Graph,
    open_target: OpenTargetFn,
    default_editor: String,
}

impl NoteViewer {
    pub fn new(
        config: WebConfig,
        open_target: OpenTargetFn,
        default_editor: impl Into<String>,
    ) -> Result<Self> {
        let graph = config.load_graph()?;
        Ok(Self {
            config,
            graph,
            open_target,
            default_editor: default_editor.into(),
        })
    }

    pub fn respond(&self, request: ViewerRequest) -> Result<ViewerResponse> {
        let state = ServeState {
            config: &self.config,
            graph: &self.graph,
            initial_target: None,
            open_target: self.open_target,
            default_editor: &self.default_editor,
        };
        Ok(http::response_for_viewer_request(
            &state,
            request.method,
            &request.path,
            request.query.as_deref(),
        )?
        .into())
    }
}

pub fn serve(
    config: &WebConfig,
    opts: &ServeOptions,
    open_target: OpenTargetFn,
    default_editor: &str,
    started: impl FnOnce(&ServeStarted) -> Result<()>,
) -> Result<()> {
    let graph = config.load_graph()?;
    let initial_target = if http::is_supported_file_path(Path::new(&opts.target)) {
        match graph.find_node(&opts.target) {
            Some(node) => InitialTarget::Org(node.uuid.clone()),
            None => InitialTarget::File(http::resolve_file_target(config, &opts.target)?),
        }
    } else {
        InitialTarget::Org(graph.resolve_target(&opts.target)?.uuid.clone())
    };
    let listener = TcpListener::bind((opts.host.as_str(), opts.port))
        .with_context(|| format!("Failed to bind {}:{}", opts.host, opts.port))?;
    let addr = listener.local_addr()?;
    let query = match &initial_target {
        InitialTarget::Org(uuid) => format!("id={}", serve::inline::percent_encode(uuid)),
        InitialTarget::File(target) => format!(
            "file={}",
            serve::inline::percent_encode(&target.request_path)
        ),
    };
    let url = format!("http://{}:{}/?{query}", addr.ip(), addr.port());

    let started_event = ServeStarted {
        url: url.clone(),
        host: addr.ip().to_string(),
        port: addr.port(),
        uuid: match &initial_target {
            InitialTarget::Org(uuid) => uuid.to_string(),
            InitialTarget::File(_) => String::new(),
        },
        path: match &initial_target {
            InitialTarget::Org(_) => None,
            InitialTarget::File(target) => Some(target.request_path.clone()),
        },
    };
    started(&started_event)?;

    let state = ServeState {
        config,
        graph: &graph,
        initial_target: Some(&initial_target),
        open_target,
        default_editor,
    };
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = http::handle_connection(stream, &state) {
                    http::log_request_error(&err);
                }
            }
            Err(err) => http::log_connection_error(&err),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "serve/viewer_tests.rs"]
mod viewer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use pkms_org::OrgConfig;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static OPENED_TARGET: Mutex<Option<(PathBuf, usize)>> = Mutex::new(None);

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

    fn record_open_target(
        _graph: &Graph,
        target: &str,
        _editor: &str,
        line: Option<usize>,
    ) -> Result<()> {
        *OPENED_TARGET.lock().unwrap() = Some((PathBuf::from(target), line.unwrap_or(1)));
        Ok(())
    }

    #[test]
    fn classifies_client_disconnect_errors() {
        for kind in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionAborted,
        ] {
            let err = anyhow::Error::new(io::Error::new(kind, "client went away"))
                .context("failed to write serve response");

            assert!(is_client_disconnect(&err), "kind should match: {kind:?}");
        }

        let other = anyhow::Error::new(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        assert!(!is_client_disconnect(&other));
    }

    #[test]
    fn page_css_uses_bundled_font_faces() {
        let css = page_css();

        assert!(css.contains("@font-face"));
        assert!(css.contains("font-family: \"PKMS Commissioner\""));
        assert!(css.contains("font-family: \"PKMS Outfit\""));
        assert!(css.contains("font-family: \"PKMS Iosevka\""));
        assert!(css.contains("font-family: \"PKMS Symbols Nerd Font Mono\""));
        assert!(css.contains("url(\"/font/Commissioner.ttf\")"));
        assert!(css.contains("url(\"/font/Outfit.ttf\")"));
        assert!(css.contains("url(\"/font/Iosevka-Regular.woff2\")"));
        assert!(css.contains("url(\"/font/Iosevka-Bold.woff2\")"));
        assert!(css.contains("url(\"/font/SymbolsNerdFontMono-Regular.woff2\")"));
        assert!(css.contains("font-display: swap;"));
    }

    #[test]
    fn serves_bundled_font_assets_by_exact_name() {
        let response = assets::font_response("Commissioner.ttf");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type.as_str(), "font/ttf");
        assert!(response.body.len() > 100_000);

        let missing = assets::font_response("../serve.rs");

        assert_eq!(missing.status, 404);
        assert_eq!(missing.content_type.as_str(), "text/plain; charset=utf-8");
    }

    #[test]
    fn renders_note_preview_hook_and_fragment() {
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
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let alpha = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let beta = graph
            .resolve_target("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb")
            .unwrap();
        let alpha_content = fs::read_to_string(&alpha.path).unwrap();
        let beta_content = fs::read_to_string(&beta.path).unwrap();

        let page = render_note_html(&graph, &config, alpha, &alpha_content);
        let preview = render_preview_html(&graph, &config, beta, &beta_content);

        assert!(page.contains("data-preview-id=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
        assert!(page.contains("id=\"note-preview\""));
        assert!(page.contains("data-preview-url=\"/preview\""));
        assert!(page.contains("class=\"note-header-actions\""));
        assert!(page.contains("class=\"open-note-button\""));
        assert!(page.contains("data-open-url=\"/open?id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\""));
        assert!(page.contains("Open in Emacs"));
        assert!(page.contains("fetch(openButton.dataset.openUrl, { method: \"POST\" })"));
        assert!(page.contains("const outlineLinks = Array.from(document.querySelectorAll"));
        assert!(page.contains("item.link.classList.toggle(\"active-outline\", active);"));
        assert!(page.contains("item.link.setAttribute(\"aria-current\", \"location\");"));
        assert!(page.contains("const HOVER_DELAY_MS = 450;"));
        assert!(page.contains("originalNote.addEventListener(\"click\""));
        assert!(page.contains("preview.addEventListener(\"wheel\""));
        assert!(page.contains("event.preventDefault();"));
        assert!(page.contains("maxScrollTop"));
        assert!(preview.contains("<article class=\"note-body note-preview-body\">"));
        assert!(preview.contains("<h2 id=\"h-6\">Preview heading</h2>"));
        assert!(preview.contains("Preview body."));
        assert!(!preview.contains("contents-panel"));
        assert!(!preview.contains("backlinks-panel"));
        assert!(!preview.contains("<html"));
    }

    #[test]
    fn renders_heading_id_links_to_note_anchor_and_heading_preview() {
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

[[id:dddddddd-dddd-4ddd-8ddd-dddddddddddd][Target section]]
"#,
        )
        .unwrap();
        fs::write(
            roam.join("b.org"),
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb
:END:
#+title: Beta

* Before
Before body.
* Target Section
:PROPERTIES:
:ID:       dddddddd-dddd-4ddd-8ddd-dddddddddddd
:END:
Target body.
** Target Child
Child body.
* Sibling
Sibling body.
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let alpha = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let target_heading = graph
            .resolve_target("dddddddd-dddd-4ddd-8ddd-dddddddddddd")
            .unwrap();
        let alpha_content = fs::read_to_string(&alpha.path).unwrap();
        let beta_content = fs::read_to_string(&target_heading.path).unwrap();

        let page = render_note_html(&graph, &config, alpha, &alpha_content);
        let preview = render_preview_html(&graph, &config, target_heading, &beta_content);

        assert!(page.contains(
            "href=\"/?id=bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb#h-8\" data-preview-id=\"dddddddd-dddd-4ddd-8ddd-dddddddddddd\""
        ));
        assert!(preview.contains("<h1>Target Section</h1>"));
        assert!(preview.contains("<h2 id=\"h-1\">Target Section</h2>"));
        assert!(preview.contains("Target body."));
        assert!(preview.contains("<h3 id=\"h-6\">Target Child</h3>"));
        assert!(preview.contains("Child body."));
        assert!(!preview.contains("<h1>Beta</h1>"));
        assert!(!preview.contains("Before body."));
        assert!(!preview.contains("Sibling body."));
    }

    #[test]
    fn open_response_uses_configured_opener() {
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

Body.
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let initial_uuid: pkms_org::domain::NoteId = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".into();
        let initial_target = InitialTarget::Org(initial_uuid);
        let state = ServeState {
            config: &config,
            graph: &graph,
            initial_target: Some(&initial_target),
            open_target: noop_open_target,
            default_editor: "true",
        };

        let response =
            open_response(&state, Some("id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")).unwrap();
        let missing = open_response(&state, None).unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type.as_str(), "text/plain; charset=utf-8");
        assert_eq!(String::from_utf8(response.body).unwrap(), "Opened Alpha");
        assert_eq!(missing.status, 404);
    }

    #[test]
    fn open_response_opens_markdown_at_the_beginning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("db");
        fs::create_dir(&root).unwrap();
        let markdown_path = dir.path().join("guide.md");
        fs::write(&markdown_path, "# Guide\n").unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let file_target = resolve_file_target(&config, markdown_path.to_str().unwrap()).unwrap();
        let initial_target = InitialTarget::File(file_target);
        let state = ServeState {
            config: &config,
            graph: &graph,
            initial_target: Some(&initial_target),
            open_target: record_open_target,
            default_editor: "emacsclient -n",
        };
        *OPENED_TARGET.lock().unwrap() = None;

        let query = format!(
            "file={}",
            percent_encode(markdown_path.canonicalize().unwrap().to_str().unwrap())
        );
        let response = open_response(&state, Some(&query)).unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(
            *OPENED_TARGET.lock().unwrap(),
            Some((markdown_path.canonicalize().unwrap(), 1))
        );
    }

    #[test]
    fn file_target_prefers_the_database_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let markdown_path = root.join("README.md");
        fs::write(&markdown_path, "# Database readme\n").unwrap();
        let config = web_config(root);

        let target = resolve_file_target(&config, "README.md").unwrap();

        assert_eq!(target.path, markdown_path.canonicalize().unwrap());
        assert_eq!(target.request_path, "README.md");
    }

    #[test]
    fn renders_outline_and_backlinks_panels() {
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

* TODO First
** DONE Child
* [[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Universal ping utility]]
"#,
        )
        .unwrap();
        fs::write(
            roam.join("b.org"),
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta

[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa][Alpha]]
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("<details class=\"side-panel contents-panel\">"));
        assert!(html.contains("<summary>Contents</summary>"));
        assert!(html.contains("href=\"#h-6\""));
        assert!(html.contains("<a href=\"#h-6\"><span class=\"todo\">TODO</span> First</a>"));
        assert!(html.contains("<h2 id=\"h-6\"><span class=\"todo\">TODO</span> First</h2>"));
        assert!(html.contains("href=\"#h-7\""));
        assert!(html.contains("<a href=\"#h-7\"><span class=\"todo\">DONE</span> Child</a>"));
        assert!(html.contains(
            "<h3 id=\"h-7\" class=\"closed-heading\"><span class=\"todo\">DONE</span> Child</h3>"
        ));
        assert!(html.contains("<a href=\"#h-8\">Universal ping utility</a>"));
        assert!(!html.contains(
            "<a href=\"#h-8\">[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Universal ping utility]]</a>"
        ));
        assert!(html.contains("<details class=\"side-panel backlinks-panel\">"));
        assert!(html.contains("<summary>Backlinks</summary>"));
        assert!(html.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
        assert!(html.contains("Beta"));
    }

    #[test]
    fn renders_only_configured_todo_states_as_todo_badges() {
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

* WAITING Plain heading
* PROBLEM [#A] Priority-looking heading
* TODO Configured open
* DONE Configured closed
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("<a href=\"#h-6\">WAITING Plain heading</a>"));
        assert!(html.contains("<h2 id=\"h-6\">WAITING Plain heading</h2>"));
        assert!(html.contains("<a href=\"#h-7\">PROBLEM [#A] Priority-looking heading</a>"));
        assert!(html.contains("<h2 id=\"h-7\">PROBLEM [#A] Priority-looking heading</h2>"));
        assert!(
            html.contains("<a href=\"#h-8\"><span class=\"todo\">TODO</span> Configured open</a>")
        );
        assert!(
            html.contains("<h2 id=\"h-8\"><span class=\"todo\">TODO</span> Configured open</h2>")
        );
        assert!(
            html.contains(
                "<a href=\"#h-9\"><span class=\"todo\">DONE</span> Configured closed</a>"
            )
        );
        assert!(html.contains(
            "<h2 id=\"h-9\" class=\"closed-heading\"><span class=\"todo\">DONE</span> Configured closed</h2>"
        ));
        assert!(!html.contains("<span class=\"todo\">WAITING</span>"));
        assert!(!html.contains("<span class=\"todo\">PROBLEM</span>"));
    }

    #[test]
    fn panel_css_uses_page_background_and_stable_backlinks_summary() {
        let css = page_css();
        let side_panel_css = css
            .split(".contents-panel")
            .next()
            .expect("side panel rules should precede contents panel");

        assert!(css.contains(".side-panel"));
        assert!(css.contains("position: fixed;"));
        assert!(css.contains("background: var(--bg);"));
        assert!(!side_panel_css.contains("border: 1px solid var(--border);"));
        assert!(css.contains(".backlinks-panel summary"));
        assert!(css.contains("justify-content: flex-end;"));
        assert!(css.contains("text-align: right;"));
        assert!(css.contains(".backlinks-panel summary::after"));
        assert!(css.contains("border-right: 0.42rem solid currentColor;"));
        assert!(css.contains(".outline-list a.active-outline"));
        assert!(css.contains("font-weight: 700;"));
        assert!(css.contains("@media (max-width: 1360px)"));
        assert!(css.contains("width: min(17rem, calc(100vw - 1.5rem));"));
        assert!(css.contains("left: 0.75rem;"));
        assert!(css.contains("right: 0.75rem;"));
        assert!(!css.contains("position: sticky;"));
        assert!(!css.contains("width: min(78ch, calc(100% - 32px));\n    max-height: none;"));

        let js = page_js();
        assert!(js.contains("window.matchMedia(\"(min-width: 1361px)\")"));
        assert!(js.contains("panel.open = widePanels.matches;"));
    }

    #[test]
    fn note_body_css_keeps_lists_tight_and_tilde_code_subdued() {
        let css = page_css();

        assert!(css.contains("ul,\nol {\n  margin: 0 0 1em;\n  padding-left: 1.45em;\n}"));
        assert!(css.contains("p:has(+ ul),\np:has(+ ol) {\n  margin-bottom: 0;\n}"));
        assert!(css.contains("li > ul,\nli > ol {\n  margin-bottom: 0;\n}"));
        assert!(css.contains(
            ".org-block-content {\n  padding: 0.8rem 0.9rem;\n  white-space: pre-wrap;\n}"
        ));
        assert!(css.contains("--tilde-code: #9a3412;"));
        assert!(css.contains("--tilde-code: #c9672c;"));
        assert!(
            css.contains(".code-orange {\n  color: var(--tilde-code);\n  font-weight: 400;\n}")
        );
        assert!(css.contains("--mention-code: #0284c7;"));
        assert!(css.contains("--mention-code: #7dd3fc;"));
        assert!(
            css.contains(".code-mention {\n  color: var(--mention-code);\n  font-weight: 400;\n}")
        );
    }

    #[test]
    fn renders_links_tables_code_math_and_images() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let roam = root.join("roam");
        fs::create_dir_all(&roam).unwrap();
        fs::create_dir_all(root.join(".attach/aa/aaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")).unwrap();
        fs::write(
            root.join(".attach/aa/aaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa/pic.png"),
            b"png",
        )
        .unwrap();
        fs::write(
            roam.join("a.org"),
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha
#+filetags: :agenda:projects:

* DONE Finished :archive:
* TODO Active :work:test:
SCHEDULED: <2026-05-27 Wed> DEADLINE: <2026-05-28 Thu>
- [x] Ticked item
- [ ] Open item
- First level
  - Second level
- Back to first
- Wrapped first line
  continued first line
  a) Alpha child first line
     continued alpha child
  b) Beta child
1. Number one
2. Number two
   1. Number two child
3. Number three

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Beta]]
[[https://example.org/docs][Example docs]]
Plain link: https://example.com/path?x=1.
[[attachment:pic.png][Picture]]

| Name | Value |
|------+-------|
| one  | $x^2$ |

#+begin_src rust
fn main() {}
#+end_src
"#,
        )
        .unwrap();
        fs::write(
            roam.join("b.org"),
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
        assert!(
            html.contains(
                "<a href=\"https://example.org/docs\" rel=\"noreferrer\">Example docs</a>"
            )
        );
        assert!(html.contains(
            "Plain link: <a href=\"https://example.com/path?x=1\" rel=\"noreferrer\">https://example.com/path?x=1</a>."
        ));
        assert!(html.contains("<div class=\"tag-list note-tags\" aria-label=\"Note tags\"><span class=\"tag\">#agenda</span><span class=\"tag\">#projects</span></div>"));
        assert!(html.contains("<h2 id=\"h-7\" class=\"closed-heading\"><span class=\"todo\">DONE</span> Finished <span class=\"heading-tags\" aria-label=\"Heading tags\"><span class=\"tag\">#archive</span></span></h2>"));
        assert!(html.contains("<h2 id=\"h-8\"><span class=\"todo\">TODO</span> Active <span class=\"heading-tags\" aria-label=\"Heading tags\"><span class=\"tag\">#work</span><span class=\"tag\">#test</span></span></h2>"));
        assert!(html.contains("<span class=\"planning planning-scheduled\"><span class=\"planning-label\">Scheduled</span> <time datetime=\"2026-05-27\">2026-05-27 Wed</time></span>"));
        assert!(html.contains("<span class=\"planning planning-deadline\"><span class=\"planning-label\">Deadline</span> <time datetime=\"2026-05-28\">2026-05-28 Thu</time></span>"));
        assert!(!html.contains("<p>SCHEDULED:"));
        assert!(!html.contains("<p>DEADLINE:"));
        assert!(html.contains("<li class=\"checked-item\">[x] Ticked item</li>"));
        assert!(html.contains("<li>[ ] Open item</li>"));
        assert!(html.contains(
            "<li>First level<ul>\n<li>Second level</li>\n</ul>\n</li>\n<li>Back to first</li>"
        ));
        assert!(html.contains(
            "<li>Wrapped first line continued first line<ol type=\"a\">\n<li>Alpha child first line continued alpha child</li>\n<li>Beta child</li>\n</ol>\n</li>"
        ));
        assert!(html.contains(
            "<ol>\n<li>Number one</li>\n<li>Number two<ol>\n<li>Number two child</li>\n</ol>\n</li>\n<li>Number three</li>\n</ol>"
        ));
        assert!(html.contains("<table>"));
        assert!(html.contains("<th scope=\"col\">Name</th>"));
        assert!(html.contains("<th scope=\"col\">Value</th>"));
        assert!(html.contains("class=\"katex\""));
        assert!(html.contains("<math"));
        assert!(html.contains("<code class=\"syn-code\">"));
        assert!(html.contains("syn-"));
        assert!(html.contains("main"));
        assert!(html.contains("<img src=\"/asset?note=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa&amp;"));
    }

    #[test]
    fn renders_org_blocks_as_identified_blocks() {
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

#+begin_src rust
fn main() {}
#+end_src

#+begin_src
plain source
#+end_src

#+begin_example
<literal example>
#+end_example

#+begin_quote
quoted *text*
second quoted line
#+end_quote

#+begin_verse
first line
second line
#+end_verse

#+caption: centered caption
#+begin_center
centered /text/
#+end_center

#+caption: comment caption
#+begin_comment
comment text
#+end_comment

#+caption: ascii caption
#+begin_export ascii
ascii <export>
#+end_export

#+caption: html caption
#+begin_export html
<script>alert("x")</script>
#+end_export

#+caption: latex caption
#+begin_export latex
\frac{a}{b}
#+end_export

#+caption: export caption
#+begin_export
generic export
#+end_export
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("org-block-src"));
        assert!(html.contains("<figcaption>Source: rust</figcaption>"));
        assert!(html.contains("<figcaption>Source</figcaption>"));
        assert!(html.contains("class=\"org-block org-block-example\""));
        assert!(html.contains("&lt;literal example&gt;"));
        assert!(html.contains("class=\"org-block org-block-quote\""));
        assert!(html.contains("quoted <strong>text</strong>\nsecond quoted line"));
        assert!(html.contains("class=\"org-block org-block-verse\""));
        assert!(html.contains("first line\nsecond line"));
        assert!(html.contains("class=\"org-block org-block-center\""));
        assert!(html.contains("centered <em>text</em>"));
        assert!(html.contains("<div class=\"org-block-caption\">centered caption</div>"));
        assert!(html.contains("class=\"org-block org-block-comment\""));
        assert!(html.contains("<figcaption>Comment</figcaption>"));
        assert!(html.contains("comment text"));
        assert!(html.contains("<div class=\"org-block-caption\">comment caption</div>"));
        assert!(html.contains("<figcaption>Export: ascii</figcaption>"));
        assert!(html.contains("ascii &lt;export&gt;"));
        assert!(html.contains("<div class=\"org-block-caption\">ascii caption</div>"));
        assert!(html.contains("<figcaption>Export: html</figcaption>"));
        assert!(html.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"));
        assert!(html.contains("<div class=\"org-block-caption\">html caption</div>"));
        assert!(html.contains("class=\"math-display\""));
        assert!(html.contains("class=\"katex-display\""));
        assert!(html.contains("<div class=\"org-block-caption\">latex caption</div>"));
        assert!(html.contains("<figcaption>Export</figcaption>"));
        assert!(html.contains("generic export"));
        assert!(html.contains("<div class=\"org-block-caption\">export caption</div>"));
    }

    #[test]
    fn percent_codec_round_trips_url_components() {
        let value = "dir/name with spaces.png";
        assert_eq!(percent_decode(&percent_encode(value)), value);
    }

    #[test]
    fn renders_org_inline_formatting() {
        let html = render_formatted_text(
            "*bold* /italic/ _under_ +gone+ =literal @skip <tag>= ~orange @skip~ `backtick @skip` @alice @bob-dev email@example.com @ $x^2$",
        );

        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<u>under</u>"));
        assert!(html.contains("<del>gone</del>"));
        assert!(html.contains("<code class=\"inline-code\">literal @skip &lt;tag&gt;</code>"));
        assert!(html.contains("<code class=\"inline-code code-orange\">orange @skip</code>"));
        assert!(html.contains("<code class=\"inline-code code-orange\">backtick @skip</code>"));
        assert!(html.contains("<code class=\"inline-code code-mention\">@alice</code>"));
        assert!(html.contains("<code class=\"inline-code code-mention\">@bob-dev</code>"));
        assert!(html.contains("email@example.com @ "));
        assert!(html.contains("class=\"katex\""));
        assert!(html.contains("<math"));
    }

    #[test]
    fn renders_plain_urls_after_org_links() {
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

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Beta]] https://example.com and xhttps://not-a-link.test
"#,
        )
        .unwrap();
        fs::write(
            roam.join("b.org"),
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
#+title: Beta
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
        assert!(html.contains(
            "<a href=\"https://example.com\" rel=\"noreferrer\">https://example.com</a>"
        ));
        assert!(!html.contains("href=\"https://not-a-link.test\""));
    }

    #[test]
    fn renders_caption_before_standalone_image() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let roam = root.join("roam");
        fs::create_dir_all(&roam).unwrap();
        fs::create_dir_all(root.join(".attach/aa/aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")).unwrap();
        fs::write(
            root.join(".attach/aa/aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa/pic.png"),
            b"png",
        )
        .unwrap();
        fs::write(
            roam.join("a.org"),
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

#+caption: *Bold* caption
[[attachment:pic.png]]

#+caption: Long caption first line
#second line
[[attachment:pic.png]]
"#,
        )
        .unwrap();
        let config = web_config(root);
        let corpus = config.load_corpus().unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("<figcaption><strong>Bold</strong> caption</figcaption>"));
        assert!(html.contains("<figcaption>Long caption first line second line</figcaption>"));
        assert!(!html.contains("<figcaption>attachment:pic.png</figcaption>"));
    }

    #[test]
    fn renders_display_math_with_katex() {
        let html = render_display_math(r"\frac{a}{b}");

        assert!(html.contains("class=\"math-display\""));
        assert!(html.contains("class=\"katex-display\""));
        assert!(html.contains("<math"));
    }

    #[test]
    fn katex_html_is_visually_hidden() {
        let css = page_css();

        assert!(css.contains(".katex .katex-html"));
        assert!(css.contains("position: absolute"));
        assert!(css.contains("clip: rect(1px, 1px, 1px, 1px)"));
    }

    #[test]
    fn code_highlighting_uses_syntect_classes() {
        let html = highlight_code("rust", "fn main() {}\n");

        assert!(html.contains("syn-"));
        assert!(html.contains("main"));
    }

    #[test]
    fn page_css_includes_syntect_rules() {
        let css = page_css();

        assert!(css.contains(".syn-code"));
        assert!(css.contains(".code pre .syn-code"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains("#c0c5ce"));
        assert!(css.contains("background-color:transparent!important"));
    }
}
