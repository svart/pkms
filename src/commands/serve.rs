use crate::commands::open;
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node, resolve_file_link_path};
use crate::output::OutputContext;
use crate::parser::{
    DEADLINE_RE, HEADING_RE, Heading, LINK_RE, SCHEDULED_RE, parse_note, strip_org_links,
};
use crate::util;
use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
static PLAIN_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>"']+"#).expect("plain URL regex is valid"));
const SYNTECT_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "syn-" };

struct ServedFont {
    name: &'static str,
    bytes: &'static [u8],
    content_type: &'static str,
}

static SERVED_FONTS: &[ServedFont] = &[
    ServedFont {
        name: "Alegreya.ttf",
        bytes: include_bytes!("serve_fonts/Alegreya.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "Alegreya-Italic.ttf",
        bytes: include_bytes!("serve_fonts/Alegreya-Italic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Regular.ttf",
        bytes: include_bytes!("serve_fonts/AlegreyaSans-Regular.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Italic.ttf",
        bytes: include_bytes!("serve_fonts/AlegreyaSans-Italic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Bold.ttf",
        bytes: include_bytes!("serve_fonts/AlegreyaSans-Bold.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-BoldItalic.ttf",
        bytes: include_bytes!("serve_fonts/AlegreyaSans-BoldItalic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "FiraCode.ttf",
        bytes: include_bytes!("serve_fonts/FiraCode.ttf"),
        content_type: "font/ttf",
    },
];

pub struct ServeOptions {
    pub target: String,
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
struct ServeStarted {
    url: String,
    host: String,
    port: u16,
    uuid: String,
}

struct ServeState<'a> {
    config: &'a ResolvedConfig,
    graph: Graph,
    initial_uuid: String,
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &ServeOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let initial_uuid = graph.resolve_target(&opts.target)?.uuid.clone();
    let listener = TcpListener::bind((opts.host.as_str(), opts.port))
        .with_context(|| format!("Failed to bind {}:{}", opts.host, opts.port))?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}:{}/?id={}", addr.ip(), addr.port(), initial_uuid);

    let started = ServeStarted {
        url: url.clone(),
        host: addr.ip().to_string(),
        port: addr.port(),
        uuid: initial_uuid.clone(),
    };
    if ctx.is_json() {
        ctx.print_json(&started)?;
    } else {
        println!("Serving {}", url);
    }
    std::io::stdout().flush()?;

    let state = ServeState {
        config,
        graph,
        initial_uuid,
    };
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_connection(stream, &state) {
                    log_request_error(&err);
                }
            }
            Err(err) => log_connection_error(&err),
        }
    }
    Ok(())
}

fn log_request_error(err: &anyhow::Error) {
    if is_client_disconnect(err) {
        tracing::debug!(error = %err, "serve client disconnected before response completed");
    } else {
        tracing::warn!(error = %err, "serve request failed");
    }
}

fn log_connection_error(err: &io::Error) {
    if is_client_disconnect_kind(err.kind()) {
        tracing::debug!(error = %err, "serve client disconnected before request handling");
    } else {
        tracing::warn!(error = %err, "serve connection failed");
    }
}

fn is_client_disconnect(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|err| is_client_disconnect_kind(err.kind()))
    })
}

fn is_client_disconnect_kind(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
    )
}

fn handle_connection(mut stream: TcpStream, state: &ServeState<'_>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" && method != "POST" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Method not allowed",
        );
    }

    let (path, query) = split_target(target);
    let response = if method == "POST" {
        match path {
            "/open" => open_response(state, query, open::DEFAULT_EDITOR),
            _ => Ok(HttpResponse::method_not_allowed("Method not allowed")),
        }
    } else if let Some(font_name) = path.strip_prefix("/font/") {
        Ok(font_response(font_name))
    } else {
        match path {
            "/" => render_response(state, query),
            "/preview" => preview_response(state, query),
            "/asset" => asset_response(state, query),
            "/open" => Ok(HttpResponse::method_not_allowed("Method not allowed")),
            _ => Ok(HttpResponse::not_found("Not found")),
        }
    }?;

    if method == "HEAD" {
        write_headers(&mut stream, response.status, response.content_type, 0)
    } else {
        write_response(
            &mut stream,
            response.status,
            response.content_type,
            &response.body,
        )
    }
}

struct HttpResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl HttpResponse {
    fn html(body: String) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: body.into_bytes(),
        }
    }

    fn not_found(message: &str) -> Self {
        Self {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }

    fn method_not_allowed(message: &str) -> Self {
        Self {
            status: 405,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }

    fn text(message: String) -> Self {
        Self {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: message.into_bytes(),
        }
    }
}

fn render_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let requested = query_param(query, "id").unwrap_or_else(|| state.initial_uuid.clone());
    let node = state.graph.resolve_target(&requested)?;
    let content = std::fs::read_to_string(&node.path)
        .with_context(|| format!("Failed to read {}", node.path.display()))?;
    Ok(HttpResponse::html(render_note_html(
        &state.graph,
        state.config,
        node,
        &content,
    )))
}

fn preview_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let Some(requested) = query_param(query, "id") else {
        return Ok(HttpResponse::not_found("Missing id"));
    };
    let node = state.graph.resolve_target(&requested)?;
    let content = std::fs::read_to_string(&node.path)
        .with_context(|| format!("Failed to read {}", node.path.display()))?;
    Ok(HttpResponse::html(render_preview_html(
        &state.graph,
        state.config,
        node,
        &content,
    )))
}

fn asset_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let Some(note_uuid) = query_param(query, "note") else {
        return Ok(HttpResponse::not_found("Missing note"));
    };
    let Some(kind) = query_param(query, "kind") else {
        return Ok(HttpResponse::not_found("Missing kind"));
    };
    let Some(target) = query_param(query, "target") else {
        return Ok(HttpResponse::not_found("Missing target"));
    };
    let note = state.graph.resolve_target(&note_uuid)?;
    let path = match kind.as_str() {
        "file" => resolve_file_link_path(&target, &note.path, state.config.resolved_db_root()),
        "attachment" => {
            resolve_existing_attachment(state.config.resolved_db_root(), &note.uuid, &target)
        }
        _ => return Ok(HttpResponse::not_found("Unknown asset kind")),
    };
    if !is_asset_allowed(&path, state.config.resolved_db_root()) || !path.is_file() {
        return Ok(HttpResponse::not_found("Asset not found"));
    }
    let body = std::fs::read(&path)?;
    Ok(HttpResponse {
        status: 200,
        content_type: mime_type(&path),
        body,
    })
}

fn open_response(
    state: &ServeState<'_>,
    query: Option<&str>,
    editor: &str,
) -> Result<HttpResponse> {
    let Some(note_uuid) = query_param(query, "id") else {
        return Ok(HttpResponse::not_found("Missing id"));
    };
    let node = state.graph.resolve_target(&note_uuid)?;
    open::open_target(&state.graph, state.config, &node.uuid, editor, Some(1))?;
    Ok(HttpResponse::text(format!("Opened {}", node.title)))
}

fn font_response(name: &str) -> HttpResponse {
    SERVED_FONTS
        .iter()
        .find(|font| font.name == name)
        .map(|font| HttpResponse {
            status: 200,
            content_type: font.content_type,
            body: font.bytes.to_vec(),
        })
        .unwrap_or_else(|| HttpResponse::not_found("Font not found"))
}

fn resolve_existing_attachment(db_root: &Path, uuid: &str, target: &str) -> PathBuf {
    let bucketed = util::resolve_attachment_path(db_root, uuid, target);
    if bucketed.exists() {
        bucketed
    } else {
        db_root.join(".attach").join(uuid).join(target)
    }
}

fn is_asset_allowed(path: &Path, db_root: &Path) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    if let Ok(canonical_root) = std::fs::canonicalize(db_root)
        && canonical_path.starts_with(canonical_root)
    {
        return true;
    }
    dirs::home_dir().is_some_and(|home| canonical_path.starts_with(home))
}

fn split_target(target: &str) -> (&str, Option<&str>) {
    match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    }
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &'static str,
    body: &[u8],
) -> Result<()> {
    write_headers(stream, status, content_type, body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn write_headers(
    stream: &mut TcpStream,
    status: u16,
    content_type: &'static str,
    content_len: usize,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {content_len}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n"
    )?;
    Ok(())
}

fn render_note_html(graph: &Graph, config: &ResolvedConfig, node: &Node, content: &str) -> String {
    let body = render_org_body(graph, config, node, content);
    let contents = render_contents_panel(config, content);
    let backlinks = render_backlinks_panel(graph, node);
    let tags = render_tag_list("Note tags", &node.filetags, "note-tags");
    let title = escape_html(&node.title);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>{}</style>
</head>
<body>
{contents}
{backlinks}
<aside id="note-preview" class="note-preview" data-preview-url="/preview" aria-live="polite" hidden></aside>
<main>
<header class="note-header">
<div class="note-header-actions">
<p class="eyebrow">pkms note</p>
<button type="button" class="open-note-button" data-open-url="/open?id={}">Open in Emacs</button>
</div>
<h1>{title}</h1>
{tags}
<p class="uuid">{}</p>
</header>
<article class="note-body">
{body}
</article>
</main>
<script>{}</script>
</body>
</html>"#,
        page_css(),
        percent_encode(&node.uuid),
        escape_html(&node.uuid),
        page_js()
    )
}

fn render_preview_html(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
    let body = render_org_body(graph, config, node, content);
    let tags = render_tag_list("Note tags", &node.filetags, "note-tags");
    let title = escape_html(&node.title);
    format!(
        "<section class=\"note-preview-content\" data-preview-note=\"{}\">\n<header class=\"note-preview-header\">\n<p class=\"eyebrow\">pkms note</p>\n<h1>{title}</h1>\n{tags}</header>\n<article class=\"note-body note-preview-body\">\n{body}</article>\n</section>\n",
        escape_html(&node.uuid)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutlineHeading {
    level: usize,
    line_number: usize,
    todo: Option<String>,
    title: String,
}

fn collect_outline_headings(config: &ResolvedConfig, content: &str) -> Vec<OutlineHeading> {
    content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let cap = HEADING_RE.captures(line)?;
            let raw_todo = cap.get(2).map(|m| m.as_str());
            let todo = raw_todo
                .filter(|state| is_configured_todo_state(config, state))
                .map(std::string::ToString::to_string);
            let title = heading_title_with_unconfigured_todo(
                raw_todo.filter(|_| todo.is_none()),
                cap.get(3).map(|m| m.as_str()),
                cap.get(4).map_or("", |m| m.as_str()),
            );
            Some(OutlineHeading {
                level: cap[1].len(),
                line_number: idx + 1,
                todo,
                title,
            })
        })
        .collect()
}

fn render_contents_panel(config: &ResolvedConfig, content: &str) -> String {
    let headings = collect_outline_headings(config, content);
    let mut html = String::from(
        "<details class=\"side-panel contents-panel\" open>\n<summary>Contents</summary>\n",
    );
    if headings.is_empty() {
        html.push_str("<p class=\"panel-empty\">No headings</p>\n");
    } else {
        html.push_str("<nav aria-label=\"Note contents\"><ol class=\"outline-list\">\n");
        for heading in headings {
            let indent = heading.level.saturating_sub(1);
            let _ = writeln!(
                html,
                "<li style=\"--outline-depth: {indent}\"><a href=\"#{}\">{}</a></li>",
                heading_anchor(heading.line_number),
                render_outline_heading_label(&heading)
            );
        }
        html.push_str("</ol></nav>\n");
    }
    html.push_str("</details>\n");
    html
}

fn render_outline_heading_label(heading: &OutlineHeading) -> String {
    let mut label = String::new();
    if let Some(todo) = heading.todo.as_deref().filter(|todo| !todo.is_empty()) {
        let _ = write!(label, "<span class=\"todo\">{}</span> ", escape_html(todo));
    }
    label.push_str(&render_formatted_text(&strip_org_links(&heading.title)));
    label
}

fn render_tag_list(label: &str, tags: &[String], class_name: &str) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut html = format!(
        "<div class=\"tag-list {class_name}\" aria-label=\"{}\">",
        escape_html(label)
    );
    for tag in tags {
        html.push_str(&render_tag(tag));
    }
    html.push_str("</div>\n");
    html
}

fn render_heading_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut html = String::from(" <span class=\"heading-tags\" aria-label=\"Heading tags\">");
    for tag in tags {
        html.push_str(&render_tag(tag));
    }
    html.push_str("</span>");
    html
}

fn render_tag(tag: &str) -> String {
    format!("<span class=\"tag\">#{}</span>", escape_html(tag))
}

fn render_heading_dates(heading: &Heading) -> String {
    if heading.scheduled.is_none() && heading.deadline.is_none() {
        return String::new();
    }
    let mut html = String::from("<div class=\"heading-meta\" aria-label=\"Heading planning\">\n");
    if let Some(scheduled) = heading.scheduled.as_deref() {
        html.push_str(&render_planning_item(
            "Scheduled",
            "planning-scheduled",
            scheduled,
        ));
    }
    if let Some(deadline) = heading.deadline.as_deref() {
        html.push_str(&render_planning_item(
            "Deadline",
            "planning-deadline",
            deadline,
        ));
    }
    html.push_str("</div>\n");
    html
}

fn render_planning_item(label: &str, class_name: &str, timestamp: &str) -> String {
    let display = display_timestamp(timestamp);
    let datetime = timestamp_date(timestamp)
        .map(|date| format!(" datetime=\"{}\"", escape_html(date)))
        .unwrap_or_default();
    format!(
        "<span class=\"planning {class_name}\"><span class=\"planning-label\">{}</span> <time{datetime}>{}</time></span>\n",
        escape_html(label),
        escape_html(&display)
    )
}

fn display_timestamp(timestamp: &str) -> String {
    timestamp
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .replace(">--<", " - ")
        .to_string()
}

fn timestamp_date(timestamp: &str) -> Option<&str> {
    let inner = timestamp.trim().strip_prefix('<')?;
    let date = inner.get(0..10)?;
    (date.len() == 10
        && date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-'))
    .then_some(date)
}

fn is_planning_line(trimmed: &str) -> bool {
    SCHEDULED_RE.is_match(trimmed) || DEADLINE_RE.is_match(trimmed)
}

fn render_backlinks_panel(graph: &Graph, node: &Node) -> String {
    let mut incoming: BTreeMap<(String, String), &Node> = BTreeMap::new();
    if let Some(backlink_uuids) = graph.backlinks.get(&node.uuid) {
        for uuid in backlink_uuids {
            if let Some(source) = graph.nodes.get(uuid) {
                incoming.insert(
                    (source.title.to_ascii_lowercase(), source.uuid.clone()),
                    source,
                );
            }
        }
    }

    let mut html = String::from(
        "<details class=\"side-panel backlinks-panel\" open>\n<summary>Backlinks</summary>\n",
    );
    if incoming.is_empty() {
        html.push_str("<p class=\"panel-empty\">No backlinks</p>\n");
    } else {
        html.push_str("<nav aria-label=\"Backlinks\"><ol class=\"backlink-list\">\n");
        for source in incoming.values() {
            let _ = writeln!(
                html,
                "<li><a href=\"/?id={}\">{}</a></li>",
                percent_encode(&source.uuid),
                escape_html(&source.title)
            );
        }
        html.push_str("</ol></nav>\n");
    }
    html.push_str("</details>\n");
    html
}

fn heading_anchor(line_number: usize) -> String {
    format!("h-{line_number}")
}

fn render_org_body(graph: &Graph, config: &ResolvedConfig, node: &Node, content: &str) -> String {
    let mut html = String::new();
    let lines: Vec<&str> = content.lines().collect();
    let parsed = parse_note(content);
    let headings_by_line: BTreeMap<usize, &Heading> = parsed
        .headings
        .iter()
        .map(|heading| (heading.line_number, heading))
        .collect();
    let mut i = 0;
    let mut paragraph: Vec<&str> = Vec::new();
    let mut list_stack: Vec<ListFrame> = Vec::new();
    let mut in_properties = false;
    let mut pending_caption: Option<String> = None;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if trimmed == ":PROPERTIES:" {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            in_properties = true;
            i += 1;
            continue;
        }
        if in_properties {
            if trimmed == ":END:" {
                in_properties = false;
            }
            i += 1;
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            i += 1;
            continue;
        }
        if lower.starts_with("#+title:") || lower.starts_with("#+filetags:") {
            i += 1;
            continue;
        }
        if is_planning_line(trimmed) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            i += 1;
            continue;
        }
        if lower.starts_with("#+caption:") {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            let mut caption = trimmed
                .split_once(':')
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default();
            i += 1;
            while i < lines.len() {
                let continuation = lines[i].trim();
                if continuation.starts_with("#+") || !continuation.starts_with('#') {
                    break;
                }
                if !caption.is_empty() {
                    caption.push(' ');
                }
                caption.push_str(continuation.trim_start_matches('#').trim());
                i += 1;
            }
            pending_caption = (!caption.is_empty()).then_some(caption);
            continue;
        }
        if let Some((block, next_i)) = read_org_block(&lines, i) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            let caption = pending_caption.take();
            html.push_str(&render_org_block(
                graph,
                config,
                node,
                &block,
                caption.as_deref(),
            ));
            i = next_i;
            continue;
        }
        if trimmed == r"\[" {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            let mut formula = String::new();
            i += 1;
            while i < lines.len() && lines[i].trim() != r"\]" {
                formula.push_str(lines[i].trim());
                formula.push('\n');
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            html.push_str(&render_display_math(formula.trim()));
            continue;
        }
        if trimmed.starts_with('|') {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            let mut table_lines = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                table_lines.push(lines[i].trim());
                i += 1;
            }
            html.push_str(&render_table(&table_lines, graph, config, node));
            continue;
        }
        if let Some(caption) = pending_caption.take()
            && let Some(figure) = render_standalone_image(config, node, trimmed, &caption)
        {
            html.push_str(&figure);
            i += 1;
            continue;
        }
        if let Some(item) = list_item(line) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            pending_caption = None;
            render_list_item(&mut html, &mut list_stack, item, graph, config, node);
            i += 1;
            continue;
        }
        if let Some(cap) = HEADING_RE.captures(line) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            let level = cap[1].len().saturating_add(1).min(6);
            let raw_todo = cap.get(2).map(|m| m.as_str());
            let todo = raw_todo
                .filter(|state| is_configured_todo_state(config, state))
                .unwrap_or_default();
            let title = heading_title_with_unconfigured_todo(
                raw_todo.filter(|_| todo.is_empty()),
                cap.get(3).map(|m| m.as_str()),
                cap.get(4).map_or("", |m| m.as_str()),
            );
            let heading = headings_by_line.get(&(i + 1)).copied();
            let anchor = heading_anchor(i + 1);
            if is_closed_todo_state(config, todo) {
                html.push_str(&format!(
                    "<h{level} id=\"{}\" class=\"closed-heading\">",
                    escape_html(&anchor)
                ));
            } else {
                html.push_str(&format!("<h{level} id=\"{}\">", escape_html(&anchor)));
            }
            if !todo.is_empty() {
                html.push_str(&format!(
                    "<span class=\"todo\">{}</span> ",
                    escape_html(todo)
                ));
            }
            html.push_str(&render_inline(graph, config, node, &title));
            if let Some(heading) = heading {
                html.push_str(&render_heading_tags(&heading.tags));
            }
            html.push_str(&format!("</h{level}>\n"));
            if let Some(heading) = heading {
                html.push_str(&render_heading_dates(heading));
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("#+") {
            pending_caption = None;
            i += 1;
            continue;
        }

        pending_caption = None;
        close_lists(&mut html, &mut list_stack);
        paragraph.push(line);
        i += 1;
    }

    flush_paragraph(&mut html, &mut paragraph, graph, config, node);
    close_lists(&mut html, &mut list_stack);
    html
}

fn flush_paragraph(
    html: &mut String,
    paragraph: &mut Vec<&str>,
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join("\n");
    html.push_str("<p>");
    html.push_str(&render_inline(graph, config, node, &text));
    html.push_str("</p>\n");
    paragraph.clear();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Ordered,
    Unordered,
}

impl ListKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Ordered => "ol",
            Self::Unordered => "ul",
        }
    }
}

struct ListFrame {
    kind: ListKind,
    indent: usize,
    open_item: bool,
}

fn render_list_item(
    html: &mut String,
    stack: &mut Vec<ListFrame>,
    item: ListItem<'_>,
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
) {
    while stack.last().is_some_and(|frame| frame.indent > item.indent) {
        close_list_frame(html, stack);
    }
    if stack
        .last()
        .is_some_and(|frame| frame.indent == item.indent && frame.kind != item.kind)
    {
        close_list_frame(html, stack);
    }
    if stack
        .last()
        .is_none_or(|frame| frame.indent < item.indent || frame.kind != item.kind)
    {
        let tag = item.kind.tag();
        let _ = writeln!(html, "<{tag}>");
        stack.push(ListFrame {
            kind: item.kind,
            indent: item.indent,
            open_item: false,
        });
    }
    if let Some(frame) = stack.last_mut() {
        if frame.open_item {
            html.push_str("</li>\n");
        }
        if item.checked {
            html.push_str("<li class=\"checked-item\">");
        } else {
            html.push_str("<li>");
        }
        frame.open_item = true;
    }
    html.push_str(&render_inline(graph, config, node, item.text));
}

fn close_lists(html: &mut String, stack: &mut Vec<ListFrame>) {
    while !stack.is_empty() {
        close_list_frame(html, stack);
    }
}

fn close_list_frame(html: &mut String, stack: &mut Vec<ListFrame>) {
    if let Some(frame) = stack.pop() {
        if frame.open_item {
            html.push_str("</li>\n");
        }
        let tag = frame.kind.tag();
        let _ = writeln!(html, "</{tag}>");
    }
}

struct OrgBlock<'a> {
    kind: String,
    args: &'a str,
    body: Vec<&'a str>,
}

fn read_org_block<'a>(lines: &[&'a str], start: usize) -> Option<(OrgBlock<'a>, usize)> {
    let trimmed = lines.get(start)?.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower.strip_prefix("#+begin_")?;
    let kind_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let kind = rest[..kind_end].to_string();
    let original_rest = trimmed.get("#+begin_".len()..)?;
    let args = original_rest
        .get(kind_end..)
        .map(str::trim)
        .unwrap_or_default();
    let end_marker = format!("#+end_{kind}");
    let mut body = Vec::new();
    let mut i = start + 1;
    while i < lines.len() && !lines[i].trim().eq_ignore_ascii_case(&end_marker) {
        body.push(lines[i]);
        i += 1;
    }
    if i < lines.len() {
        i += 1;
    }
    Some((OrgBlock { kind, args, body }, i))
}

fn render_org_block(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    block: &OrgBlock<'_>,
    caption: Option<&str>,
) -> String {
    match block.kind.as_str() {
        "src" => render_src_block(block, caption),
        "example" => render_pre_block("example", "Example", &block.body_text(), caption),
        "quote" => render_text_block(graph, config, node, "quote", "Quote", &block.body, caption),
        "verse" => render_pre_block("verse", "Verse", &block.body_text(), caption),
        "center" => render_text_block(
            graph,
            config,
            node,
            "center",
            "Center",
            &block.body,
            caption,
        ),
        "comment" => render_text_block(
            graph,
            config,
            node,
            "comment",
            "Comment",
            &block.body,
            caption,
        ),
        "export" => render_export_block(block, caption),
        kind => {
            let label = format!("Block: {kind}");
            render_pre_block("special", &label, &block.body_text(), caption)
        }
    }
}

impl OrgBlock<'_> {
    fn body_text(&self) -> String {
        let mut text = self.body.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    }
}

fn render_src_block(block: &OrgBlock<'_>, caption: Option<&str>) -> String {
    let lang = block.args.split_whitespace().next().unwrap_or_default();
    let label = if lang.is_empty() {
        "Source".to_string()
    } else {
        format!("Source: {lang}")
    };
    let code = block.body_text();
    let rendered_code = if lang.is_empty() {
        escape_html(&code)
    } else {
        highlight_code(lang, &code)
    };
    let mut html = format!(
        "<figure class=\"org-block org-block-src code\"><figcaption>{}</figcaption><pre><code class=\"syn-code\">{}</code></pre>",
        escape_html(&label),
        rendered_code
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn render_export_block(block: &OrgBlock<'_>, caption: Option<&str>) -> String {
    let backend = block.args.split_whitespace().next().unwrap_or_default();
    let text = block.body_text();
    if backend.eq_ignore_ascii_case("latex") {
        let mut html = format!(
            "<figure class=\"org-block org-block-export org-block-export-latex\"><figcaption>Export: latex</figcaption>{}",
            render_display_math(text.trim())
        );
        push_block_caption(&mut html, caption);
        html.push_str("</figure>\n");
        return html;
    }
    let label = if backend.is_empty() {
        "Export".to_string()
    } else {
        format!("Export: {backend}")
    };
    render_pre_block("export", &label, &text, caption)
}

fn render_pre_block(kind: &str, label: &str, text: &str, caption: Option<&str>) -> String {
    let mut html = format!(
        "<figure class=\"org-block org-block-{kind}\"><figcaption>{}</figcaption><pre>{}</pre>",
        escape_html(label),
        escape_html(text)
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn render_text_block(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    kind: &str,
    label: &str,
    lines: &[&str],
    caption: Option<&str>,
) -> String {
    let text = lines.join("\n");
    let mut html = format!(
        "<figure class=\"org-block org-block-{kind}\"><figcaption>{}</figcaption><div class=\"org-block-content\">{}</div>",
        escape_html(label),
        render_inline(graph, config, node, &text)
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn push_block_caption(html: &mut String, caption: Option<&str>) {
    if let Some(caption) = caption {
        html.push_str("<div class=\"org-block-caption\">");
        html.push_str(&render_formatted_text(caption));
        html.push_str("</div>");
    }
}

fn render_table(lines: &[&str], graph: &Graph, config: &ResolvedConfig, node: &Node) -> String {
    let mut html = String::from("<table>\n<tbody>\n");
    let mut is_header = true;
    for line in lines {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == '+'))
        {
            continue;
        }
        html.push_str("<tr>");
        for cell in cells {
            if is_header {
                html.push_str("<th scope=\"col\">");
            } else {
                html.push_str("<td>");
            }
            html.push_str(&render_inline(graph, config, node, cell));
            if is_header {
                html.push_str("</th>");
            } else {
                html.push_str("</td>");
            }
        }
        html.push_str("</tr>\n");
        is_header = false;
    }
    html.push_str("</tbody>\n</table>\n");
    html
}

struct ListItem<'a> {
    indent: usize,
    kind: ListKind,
    text: &'a str,
    checked: bool,
}

fn list_item(line: &str) -> Option<ListItem<'_>> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let trimmed = line.trim_start();
    for marker in ["- ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some(ListItem {
                indent,
                kind: ListKind::Unordered,
                text: rest,
                checked: is_checked_item(rest),
            });
        }
    }
    static ORDERED_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d+[\.)]\s+(.+)$").unwrap());
    ORDERED_RE
        .captures(trimmed)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .map(|text| ListItem {
            indent,
            kind: ListKind::Ordered,
            text,
            checked: is_checked_item(text),
        })
}

fn is_checked_item(text: &str) -> bool {
    text.get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("[x]"))
        && text.as_bytes().get(3).is_none_or(u8::is_ascii_whitespace)
}

fn is_closed_todo_state(config: &ResolvedConfig, state: &str) -> bool {
    !state.is_empty()
        && config
            .closed_todo_states()
            .iter()
            .any(|closed| closed.eq_ignore_ascii_case(state))
}

fn is_configured_todo_state(config: &ResolvedConfig, state: &str) -> bool {
    !state.is_empty()
        && (config
            .open_todo_states()
            .iter()
            .any(|open| open.eq_ignore_ascii_case(state))
            || is_closed_todo_state(config, state))
}

fn heading_title_with_unconfigured_todo(
    raw_todo: Option<&str>,
    priority: Option<&str>,
    title: &str,
) -> String {
    let Some(raw_todo) = raw_todo else {
        return title.to_string();
    };
    let mut restored = raw_todo.to_string();
    if let Some(priority) = priority {
        let _ = write!(restored, " [#{priority}]");
    }
    if !title.is_empty() {
        restored.push(' ');
        restored.push_str(title);
    }
    restored
}

fn render_inline(graph: &Graph, config: &ResolvedConfig, node: &Node, text: &str) -> String {
    let mut html = String::new();
    let mut last = 0;
    for cap in LINK_RE.captures_iter(text) {
        let Some(m) = cap.get(0) else {
            continue;
        };
        html.push_str(&render_formatted_text_with_plain_links(
            &text[last..m.start()],
        ));
        let target = cap.get(1).map_or("", |m| m.as_str());
        let desc = cap.get(2).map(|m| m.as_str()).filter(|s| !s.is_empty());
        html.push_str(&render_link(graph, config, node, target, desc));
        last = m.end();
    }
    html.push_str(&render_formatted_text_with_plain_links(&text[last..]));
    html
}

fn render_link(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    target: &str,
    desc: Option<&str>,
) -> String {
    let label = desc.unwrap_or(target);
    if let Some(uuid) = target.strip_prefix("id:") {
        let href_uuid = graph
            .resolve_target(uuid)
            .map(|n| n.uuid.as_str())
            .unwrap_or(uuid);
        return format!(
            "<a href=\"/?id={}\" data-preview-id=\"{}\">{}</a>",
            percent_encode(href_uuid),
            escape_html(href_uuid),
            render_formatted_text(label)
        );
    }
    if let Some(path) = target.strip_prefix("file:") {
        let resolved = resolve_file_link_path(path, &node.path, config.resolved_db_root());
        let href = asset_href(&node.uuid, "file", path);
        if is_image_path(&resolved) {
            return format!(
                "<figure><img src=\"{href}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
                escape_html(label),
                render_formatted_text(label)
            );
        }
        return format!("<a href=\"{href}\">{}</a>", render_formatted_text(label));
    }
    if let Some(path) = target.strip_prefix("attachment:") {
        let resolved = resolve_existing_attachment(config.resolved_db_root(), &node.uuid, path);
        let href = asset_href(&node.uuid, "attachment", path);
        if is_image_path(&resolved) {
            return format!(
                "<figure><img src=\"{href}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
                escape_html(label),
                render_formatted_text(label)
            );
        }
        return format!("<a href=\"{href}\">{}</a>", render_formatted_text(label));
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        return format!(
            "<a href=\"{}\" rel=\"noreferrer\">{}</a>",
            escape_html(target),
            render_formatted_text(label)
        );
    }
    render_formatted_text(label)
}

fn render_standalone_image(
    config: &ResolvedConfig,
    node: &Node,
    text: &str,
    caption: &str,
) -> Option<String> {
    let cap = LINK_RE.captures(text)?;
    let link = cap.get(0)?;
    if link.as_str() != text {
        return None;
    }
    let target = cap.get(1).map_or("", |m| m.as_str());
    let desc = cap.get(2).map(|m| m.as_str()).filter(|s| !s.is_empty());
    render_image_link(config, node, target, desc, Some(caption))
}

fn render_image_link(
    config: &ResolvedConfig,
    node: &Node,
    target: &str,
    desc: Option<&str>,
    caption: Option<&str>,
) -> Option<String> {
    if let Some(path) = target.strip_prefix("file:") {
        let resolved = resolve_file_link_path(path, &node.path, config.resolved_db_root());
        if is_image_path(&resolved) {
            return Some(render_image_figure(
                &asset_href(&node.uuid, "file", path),
                caption.or(desc).unwrap_or(target),
            ));
        }
    }
    if let Some(path) = target.strip_prefix("attachment:") {
        let resolved = resolve_existing_attachment(config.resolved_db_root(), &node.uuid, path);
        if is_image_path(&resolved) {
            return Some(render_image_figure(
                &asset_href(&node.uuid, "attachment", path),
                caption.or(desc).unwrap_or(target),
            ));
        }
    }
    None
}

fn render_image_figure(href: &str, caption: &str) -> String {
    format!(
        "<figure><img src=\"{href}\" alt=\"{}\"><figcaption>{}</figcaption></figure>\n",
        escape_html(caption),
        render_formatted_text(caption)
    )
}

fn asset_href(note_uuid: &str, kind: &str, target: &str) -> String {
    format!(
        "/asset?note={}&amp;kind={}&amp;target={}",
        percent_encode(note_uuid),
        percent_encode(kind),
        percent_encode(target)
    )
}

fn render_formatted_text(text: &str) -> String {
    let mut html = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('$') {
        html.push_str(&render_org_markup(&rest[..start]));
        let after = &rest[start + 1..];
        if let Some(end) = after.find('$') {
            html.push_str(&render_inline_math(&after[..end]));
            rest = &after[end + 1..];
        } else {
            html.push('$');
            html.push_str(&escape_html(after));
            return html;
        }
    }
    html.push_str(&render_org_markup(rest));
    html
}

fn render_formatted_text_with_plain_links(text: &str) -> String {
    let mut html = String::new();
    let mut last = 0;
    for m in PLAIN_URL_RE.find_iter(text) {
        if !starts_plain_url(text, m.start()) {
            continue;
        }
        let url_end = trim_plain_url_end(m.as_str());
        if url_end == 0 {
            continue;
        }
        let link_end = m.start() + url_end;
        html.push_str(&render_formatted_text(&text[last..m.start()]));
        let url = &text[m.start()..link_end];
        html.push_str(&format!(
            "<a href=\"{}\" rel=\"noreferrer\">{}</a>",
            escape_html(url),
            escape_html(url)
        ));
        last = link_end;
    }
    html.push_str(&render_formatted_text(&text[last..]));
    html
}

fn starts_plain_url(text: &str, start: usize) -> bool {
    text[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | '<'))
}

fn trim_plain_url_end(url: &str) -> usize {
    url.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}'])
        .len()
}

fn render_inline_math(input: &str) -> String {
    render_katex(input, false)
}

fn render_display_math(input: &str) -> String {
    format!(
        "<div class=\"math-display\">{}</div>\n",
        render_katex(input, true)
    )
}

fn render_katex(input: &str, display_mode: bool) -> String {
    let mut opts = katex::Opts::default();
    opts.set_display_mode(display_mode);
    opts.set_throw_on_error(false);
    opts.set_output_type(katex::OutputType::HtmlAndMathml);
    katex::render_with_opts(input, &opts).unwrap_or_else(|_| {
        format!(
            "<span class=\"math math-error\">{}</span>",
            escape_html(input)
        )
    })
}

fn render_org_markup(text: &str) -> String {
    let Some((start, end, style)) = find_emphasis(text) else {
        return render_mentions(text);
    };
    let mut html = String::new();
    html.push_str(&render_mentions(&text[..start]));
    let inner_start = start + style.delimiter().len_utf8();
    let inner = &text[inner_start..end];
    match style {
        InlineStyle::Bold => {
            html.push_str("<strong>");
            html.push_str(&render_org_markup(inner));
            html.push_str("</strong>");
        }
        InlineStyle::Italic => {
            html.push_str("<em>");
            html.push_str(&render_org_markup(inner));
            html.push_str("</em>");
        }
        InlineStyle::Underline => {
            html.push_str("<u>");
            html.push_str(&render_org_markup(inner));
            html.push_str("</u>");
        }
        InlineStyle::Strike => {
            html.push_str("<del>");
            html.push_str(&render_org_markup(inner));
            html.push_str("</del>");
        }
        InlineStyle::Verbatim => {
            html.push_str("<code class=\"inline-code\">");
            html.push_str(&escape_html(inner));
            html.push_str("</code>");
        }
        InlineStyle::OrangeCode => {
            html.push_str("<code class=\"inline-code code-orange\">");
            html.push_str(&escape_html(inner));
            html.push_str("</code>");
        }
    }
    html.push_str(&render_org_markup(
        &text[end + style.delimiter().len_utf8()..],
    ));
    html
}

fn render_mentions(text: &str) -> String {
    let mut html = String::new();
    let mut last = 0;
    for (start, _) in text.match_indices('@') {
        if !is_valid_mention_start(text, start) {
            continue;
        }
        let end = mention_end(text, start + '@'.len_utf8());
        if end == start + '@'.len_utf8() {
            continue;
        }
        html.push_str(&escape_html(&text[last..start]));
        html.push_str("<code class=\"inline-code code-mention\">");
        html.push_str(&escape_html(&text[start..end]));
        html.push_str("</code>");
        last = end;
    }
    html.push_str(&escape_html(&text[last..]));
    html
}

fn is_valid_mention_start(text: &str, start: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[start + '@'.len_utf8()..].chars().next();
    before.is_none_or(is_mention_boundary) && after.is_some_and(is_mention_char)
}

fn mention_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, c) in text[start..].char_indices() {
        if !is_mention_char(c) {
            break;
        }
        end = start + offset + c.len_utf8();
    }
    end
}

fn is_mention_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

fn is_mention_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | '[' | '{' | '<' | '\'' | '"' | ',' | ';' | ':' | '!' | '?'
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineStyle {
    Bold,
    Italic,
    Underline,
    Strike,
    Verbatim,
    OrangeCode,
}

impl InlineStyle {
    fn delimiter(self) -> char {
        match self {
            InlineStyle::Bold => '*',
            InlineStyle::Italic => '/',
            InlineStyle::Underline => '_',
            InlineStyle::Strike => '+',
            InlineStyle::Verbatim => '=',
            InlineStyle::OrangeCode => '~',
        }
    }

    fn from_delimiter(delimiter: char) -> Option<Self> {
        match delimiter {
            '*' => Some(InlineStyle::Bold),
            '/' => Some(InlineStyle::Italic),
            '_' => Some(InlineStyle::Underline),
            '+' => Some(InlineStyle::Strike),
            '=' => Some(InlineStyle::Verbatim),
            '~' => Some(InlineStyle::OrangeCode),
            _ => None,
        }
    }
}

fn find_emphasis(text: &str) -> Option<(usize, usize, InlineStyle)> {
    for (start, delimiter) in text.char_indices() {
        let Some(style) = InlineStyle::from_delimiter(delimiter) else {
            continue;
        };
        if !is_valid_emphasis_start(text, start, delimiter) {
            continue;
        }
        let search_start = start + delimiter.len_utf8();
        for (offset, candidate) in text[search_start..].char_indices() {
            if candidate != delimiter {
                continue;
            }
            let end = search_start + offset;
            if is_valid_emphasis_end(text, end, delimiter) {
                return Some((start, end, style));
            }
        }
    }
    None
}

fn is_valid_emphasis_start(text: &str, start: usize, delimiter: char) -> bool {
    if delimiter == '/' && text[start..].starts_with("//") {
        return false;
    }
    let before = text[..start].chars().next_back();
    let after = text[start + delimiter.len_utf8()..].chars().next();
    let starts_after_boundary = before.is_none_or(is_emphasis_boundary);
    let has_content = after.is_some_and(|c| !c.is_whitespace() && c != delimiter);
    starts_after_boundary && has_content
}

fn is_valid_emphasis_end(text: &str, end: usize, delimiter: char) -> bool {
    if delimiter == '/' && text[end..].starts_with("//") {
        return false;
    }
    let before = text[..end].chars().next_back();
    let after = text[end + delimiter.len_utf8()..].chars().next();
    let ends_before_boundary = after.is_none_or(is_emphasis_boundary);
    let has_content = before.is_some_and(|c| !c.is_whitespace() && c != delimiter);
    ends_before_boundary && has_content
}

fn is_emphasis_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '\''
                | '"'
                | ','
                | ';'
                | ':'
                | '.'
                | '!'
                | '?'
        )
}

fn highlight_code(lang: &str, code: &str) -> String {
    let syntax_set = &SYNTAX_SET;
    let syntax = syntax_for_lang(lang, syntax_set);
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, SYNTECT_CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return escape_html(code);
        }
    }
    generator.finalize()
}

fn syntax_for_lang<'a>(lang: &str, syntax_set: &'a SyntaxSet) -> &'a SyntaxReference {
    let lower = lang.to_ascii_lowercase();
    let token = match lower.as_str() {
        "bash" | "shell" => "sh",
        "c++" | "cxx" => "cpp",
        "emacs-lisp" | "elisp" => "el",
        "javascript" => "js",
        "python" => "py",
        "rust" => "rs",
        "typescript" => "ts",
        _ => lower.as_str(),
    };
    syntax_set
        .find_syntax_by_token(token)
        .or_else(|| syntax_set.find_syntax_by_extension(token))
        .or_else(|| syntax_set.find_syntax_by_name(lang))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn syntect_theme() -> &'static Theme {
    THEME_SET
        .themes
        .get("InspiredGitHub")
        .or_else(|| THEME_SET.themes.get("base16-ocean.dark"))
        .or_else(|| THEME_SET.themes.values().next())
        .expect("syntect default themes should include at least one theme")
}

fn syntect_css() -> String {
    let mut css =
        css_for_theme_with_class_style(syntect_theme(), SYNTECT_CLASS_STYLE).unwrap_or_default();
    css.push_str(
        ".code pre .syn-code,.code pre .syn-code span{background:transparent!important;background-color:transparent!important}",
    );
    css
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            decoded.push(byte);
            i += 3;
            continue;
        }
        decoded.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&decoded).to_string()
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
            )
        })
}

fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn page_css() -> String {
    let mut css = include_str!("serve.css").to_string();
    css.push('\n');
    css.push_str(&syntect_css());
    css
}

fn page_js() -> &'static str {
    r#"(() => {
  const HOVER_DELAY_MS = 450;
  const preview = document.getElementById("note-preview");
  const originalNote = document.querySelector(".note-body");
  const outlineLinks = Array.from(document.querySelectorAll(".contents-panel a[href^='#h-']"));
  const openButton = document.querySelector(".open-note-button[data-open-url]");

  if (originalNote && outlineLinks.length > 0) {
    const headings = outlineLinks
      .map((link) => {
        const id = link.getAttribute("href").slice(1);
        const heading = document.getElementById(id);
        return heading ? { id, heading, link } : null;
      })
      .filter(Boolean);
    let activeId = "";
    let ticking = false;

    function setActiveOutline(id) {
      if (id === activeId) {
        return;
      }
      activeId = id;
      for (const item of headings) {
        const active = item.id === id;
        item.link.classList.toggle("active-outline", active);
        if (active) {
          item.link.setAttribute("aria-current", "location");
        } else {
          item.link.removeAttribute("aria-current");
        }
      }
    }

    function updateActiveOutline() {
      ticking = false;
      if (headings.length === 0) {
        return;
      }
      const threshold = Math.min(window.innerHeight * 0.3, 160);
      let current = headings[0];
      for (const item of headings) {
        if (item.heading.getBoundingClientRect().top <= threshold) {
          current = item;
        } else {
          break;
        }
      }
      setActiveOutline(current.id);
    }

    function requestOutlineUpdate() {
      if (ticking) {
        return;
      }
      ticking = true;
      window.requestAnimationFrame(updateActiveOutline);
    }

    window.addEventListener("scroll", requestOutlineUpdate, { passive: true });
    window.addEventListener("resize", requestOutlineUpdate);
    updateActiveOutline();
  }

  if (openButton && window.fetch) {
    openButton.addEventListener("click", async () => {
      const label = openButton.textContent;
      openButton.disabled = true;
      try {
        const response = await fetch(openButton.dataset.openUrl, { method: "POST" });
        openButton.textContent = response.ok ? "Opened" : "Open failed";
      } catch (_error) {
        openButton.textContent = "Open failed";
      } finally {
        window.setTimeout(() => {
          openButton.textContent = label;
          openButton.disabled = false;
        }, 1400);
      }
    });
  }
  if (!preview || !originalNote || !window.fetch) {
    return;
  }

  let hoverTimer = 0;
  let activeController = null;
  const cache = new Map();

  function hidePreview() {
    window.clearTimeout(hoverTimer);
    hoverTimer = 0;
    if (activeController) {
      activeController.abort();
      activeController = null;
    }
    preview.hidden = true;
    preview.innerHTML = "";
    preview.removeAttribute("data-active-preview");
  }

  function positionPreview(anchor) {
    const rect = anchor.getBoundingClientRect();
    preview.style.top = `${Math.max(16, Math.min(rect.top, window.innerHeight * 0.35))}px`;
  }

  async function showPreview(anchor) {
    const uuid = anchor.dataset.previewId;
    if (!uuid) {
      return;
    }
    positionPreview(anchor);
    preview.hidden = false;
    preview.dataset.activePreview = uuid;
    preview.innerHTML = "<p class=\"panel-empty\">Loading...</p>";

    if (cache.has(uuid)) {
      preview.innerHTML = cache.get(uuid);
      return;
    }

    if (activeController) {
      activeController.abort();
    }
    activeController = new AbortController();
    const url = `${preview.dataset.previewUrl}?id=${encodeURIComponent(uuid)}`;
    try {
      const response = await fetch(url, { signal: activeController.signal });
      if (!response.ok) {
        throw new Error(`Preview request failed: ${response.status}`);
      }
      const html = await response.text();
      cache.set(uuid, html);
      if (preview.dataset.activePreview === uuid) {
        preview.innerHTML = html;
      }
    } catch (error) {
      if (error.name !== "AbortError" && preview.dataset.activePreview === uuid) {
        preview.innerHTML = "<p class=\"panel-empty\">Preview unavailable</p>";
      }
    }
  }

  document.addEventListener("mouseover", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target ? target.closest("a[data-preview-id]") : null;
    if (!anchor) {
      return;
    }
    window.clearTimeout(hoverTimer);
    hoverTimer = window.setTimeout(() => showPreview(anchor), HOVER_DELAY_MS);
  });

  document.addEventListener("mouseout", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target ? target.closest("a[data-preview-id]") : null;
    if (!anchor || anchor.contains(event.relatedTarget)) {
      return;
    }
    window.clearTimeout(hoverTimer);
  });

  originalNote.addEventListener("click", (event) => {
    if (!preview.hidden && !preview.contains(event.target)) {
      hidePreview();
    }
  });

  preview.addEventListener("wheel", (event) => {
    event.stopPropagation();
    const lineHeight = 16;
    const pageHeight = preview.clientHeight;
    const delta = event.deltaMode === 1
      ? event.deltaY * lineHeight
      : event.deltaMode === 2
        ? event.deltaY * pageHeight
        : event.deltaY;
    const maxScrollTop = preview.scrollHeight - preview.clientHeight;
    const atTop = preview.scrollTop <= 0;
    const atBottom = preview.scrollTop >= maxScrollTop - 1;
    const targetScrollTop = preview.scrollTop + delta;
    if (delta < 0 && (atTop || targetScrollTop < 0)) {
      event.preventDefault();
      preview.scrollTop = 0;
    } else if (delta > 0 && (atBottom || targetScrollTop > maxScrollTop)) {
      event.preventDefault();
      preview.scrollTop = maxScrollTop;
    }
  }, { passive: false });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      hidePreview();
    }
  });
})();"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use crate::corpus::Corpus;
    use std::fs;

    fn test_config(root: PathBuf) -> ResolvedConfig {
        ResolvedConfig {
            db_root: root,
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
        }
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
        assert!(css.contains("font-family: \"PKMS Alegreya\""));
        assert!(css.contains("font-family: \"PKMS Alegreya Sans\""));
        assert!(css.contains("font-family: \"PKMS Fira Code\""));
        assert!(css.contains("url(\"/font/Alegreya.ttf\")"));
        assert!(css.contains("url(\"/font/Alegreya-Italic.ttf\")"));
        assert!(css.contains("url(\"/font/AlegreyaSans-Regular.ttf\")"));
        assert!(css.contains("url(\"/font/AlegreyaSans-Bold.ttf\")"));
        assert!(css.contains("url(\"/font/FiraCode.ttf\")"));
        assert!(css.contains("font-display: swap;"));
    }

    #[test]
    fn serves_bundled_font_assets_by_exact_name() {
        let response = font_response("Alegreya.ttf");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "font/ttf");
        assert!(response.body.len() > 100_000);

        let missing = font_response("../serve.rs");

        assert_eq!(missing.status, 404);
        assert_eq!(missing.content_type, "text/plain; charset=utf-8");
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
    fn open_response_uses_existing_editor_opening() {
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
        let graph = Graph::from_corpus(&corpus);
        let state = ServeState {
            config: &config,
            graph,
            initial_uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
        };

        let response = open_response(
            &state,
            Some("id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"),
            "true",
        )
        .unwrap();
        let missing = open_response(&state, None, "true").unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/plain; charset=utf-8");
        assert_eq!(String::from_utf8(response.body).unwrap(), "Opened Alpha");
        assert_eq!(missing.status, 404);
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
        let graph = Graph::from_corpus(&corpus);
        let node = graph
            .resolve_target("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            .unwrap();
        let content = fs::read_to_string(&node.path).unwrap();

        let html = render_note_html(&graph, &config, node, &content);

        assert!(html.contains("<details class=\"side-panel contents-panel\" open>"));
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
        assert!(html.contains("<details class=\"side-panel backlinks-panel\" open>"));
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
        assert!(css.contains("background: var(--bg);"));
        assert!(!side_panel_css.contains("border: 1px solid var(--border);"));
        assert!(css.contains(".backlinks-panel summary"));
        assert!(css.contains("justify-content: flex-end;"));
        assert!(css.contains("text-align: right;"));
        assert!(css.contains(".backlinks-panel summary::after"));
        assert!(css.contains("border-right: 0.42rem solid currentColor;"));
        assert!(css.contains(".outline-list a.active-outline"));
        assert!(css.contains("font-weight: 700;"));
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
            "*bold* /italic/ _under_ +gone+ =literal @skip <tag>= ~orange @skip~ @alice @bob-dev email@example.com @ $x^2$",
        );

        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<u>under</u>"));
        assert!(html.contains("<del>gone</del>"));
        assert!(html.contains("<code class=\"inline-code\">literal @skip &lt;tag&gt;</code>"));
        assert!(html.contains("<code class=\"inline-code code-orange\">orange @skip</code>"));
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
        let config = test_config(root);
        let corpus = Corpus::load(&config).unwrap();
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
        assert!(css.contains("background-color:transparent!important"));
    }
}
