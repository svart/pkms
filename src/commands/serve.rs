use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node, resolve_file_link_path};
use crate::output::OutputContext;
use crate::parser::{HEADING_RE, LINK_RE};
use crate::util;
use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
const SYNTECT_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "syn-" };

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
                    eprintln!("serve request failed: {err}");
                }
            }
            Err(err) => eprintln!("serve connection failed: {err}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, state: &ServeState<'_>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Method not allowed",
        );
    }

    let (path, query) = split_target(target);
    let response = match path {
        "/" => render_response(state, query),
        "/asset" => asset_response(state, query),
        _ => Ok(HttpResponse::not_found("Not found")),
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
<main>
<header class="note-header">
<p class="eyebrow">pkms note</p>
<h1>{title}</h1>
<p class="uuid">{}</p>
</header>
<article class="note-body">
{body}
</article>
</main>
</body>
</html>"#,
        page_css(),
        escape_html(&node.uuid)
    )
}

fn render_org_body(graph: &Graph, config: &ResolvedConfig, node: &Node, content: &str) -> String {
    let mut html = String::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut paragraph: Vec<&str> = Vec::new();
    let mut in_list = false;
    let mut in_properties = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if trimmed == ":PROPERTIES:" {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_list(&mut html, &mut in_list);
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
            close_list(&mut html, &mut in_list);
            i += 1;
            continue;
        }
        if lower.starts_with("#+title:") || lower.starts_with("#+filetags:") {
            i += 1;
            continue;
        }
        if lower.starts_with("#+begin_src") {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_list(&mut html, &mut in_list);
            let lang = trimmed.split_whitespace().nth(1).unwrap_or("text");
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().eq_ignore_ascii_case("#+end_src") {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            html.push_str(&format!(
                "<figure class=\"code\"><figcaption>{}</figcaption><pre><code class=\"syn-code\">{}</code></pre></figure>\n",
                escape_html(lang),
                highlight_code(lang, &code)
            ));
            continue;
        }
        if lower.starts_with("#+begin_export latex") || trimmed == r"\[" {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_list(&mut html, &mut in_list);
            let mut formula = String::new();
            i += 1;
            while i < lines.len()
                && !lines[i].trim().eq_ignore_ascii_case("#+end_export")
                && lines[i].trim() != r"\]"
            {
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
            close_list(&mut html, &mut in_list);
            let mut table_lines = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                table_lines.push(lines[i].trim());
                i += 1;
            }
            html.push_str(&render_table(&table_lines, graph, config, node));
            continue;
        }
        if let Some(item) = list_item_text(trimmed) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            html.push_str("<li>");
            html.push_str(&render_inline(graph, config, node, item));
            html.push_str("</li>\n");
            i += 1;
            continue;
        }
        if let Some(cap) = HEADING_RE.captures(line) {
            flush_paragraph(&mut html, &mut paragraph, graph, config, node);
            close_list(&mut html, &mut in_list);
            let level = cap[1].len().saturating_add(1).min(6);
            let todo = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
            let title = cap.get(4).map_or("", |m| m.as_str());
            html.push_str(&format!("<h{level}>"));
            if !todo.is_empty() {
                html.push_str(&format!(
                    "<span class=\"todo\">{}</span> ",
                    escape_html(todo)
                ));
            }
            html.push_str(&render_inline(graph, config, node, title));
            html.push_str(&format!("</h{level}>\n"));
            i += 1;
            continue;
        }
        if trimmed.starts_with("#+") {
            i += 1;
            continue;
        }

        paragraph.push(line);
        i += 1;
    }

    flush_paragraph(&mut html, &mut paragraph, graph, config, node);
    close_list(&mut html, &mut in_list);
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

fn close_list(html: &mut String, in_list: &mut bool) {
    if *in_list {
        html.push_str("</ul>\n");
        *in_list = false;
    }
}

fn render_table(lines: &[&str], graph: &Graph, config: &ResolvedConfig, node: &Node) -> String {
    let mut html = String::from("<table>\n<tbody>\n");
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
            html.push_str("<td>");
            html.push_str(&render_inline(graph, config, node, cell));
            html.push_str("</td>");
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</tbody>\n</table>\n");
    html
}

fn list_item_text(trimmed: &str) -> Option<&str> {
    for marker in ["- ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some(rest);
        }
    }
    static ORDERED_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d+[\.)]\s+(.+)$").unwrap());
    ORDERED_RE
        .captures(trimmed)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
}

fn render_inline(graph: &Graph, config: &ResolvedConfig, node: &Node, text: &str) -> String {
    let mut html = String::new();
    let mut last = 0;
    for cap in LINK_RE.captures_iter(text) {
        let Some(m) = cap.get(0) else {
            continue;
        };
        html.push_str(&render_formatted_text(&text[last..m.start()]));
        let target = cap.get(1).map_or("", |m| m.as_str());
        let desc = cap.get(2).map(|m| m.as_str()).filter(|s| !s.is_empty());
        html.push_str(&render_link(graph, config, node, target, desc));
        last = m.end();
    }
    html.push_str(&render_formatted_text(&text[last..]));
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
            "<a href=\"/?id={}\">{}</a>",
            percent_encode(href_uuid),
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
        return escape_html(text);
    };
    let mut html = String::new();
    html.push_str(&escape_html(&text[..start]));
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
    let mut css = r#":root{color-scheme:light dark;--bg:#fafafa;--fg:#1f2328;--muted:#667085;--border:#d0d7de;--surface:#fff;--accent:#0969da;--code:#f6f8fa;--mark:#fff7cc;--inline-code:#f3f4f6;--orange-code:#bf360c}
@media (prefers-color-scheme:dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#8b949e;--border:#30363d;--surface:#161b22;--accent:#58a6ff;--code:#161b22;--mark:#3b3200;--inline-code:#1f2937;--orange-code:#ff9f5a}}
*{box-sizing:border-box} body{margin:0;background:var(--bg);color:var(--fg);font:18px/1.68 Alegreya,"Iowan Old Style",Palatino,Georgia,serif} main{width:min(78ch,calc(100% - 32px));margin:0 auto;padding:40px 0 64px}.note-header{border-bottom:1px solid var(--border);margin-bottom:28px;padding-bottom:20px}.eyebrow{color:var(--muted);font:600 12px/1.2 "Alegreya Sans",ui-sans-serif,system-ui,sans-serif;letter-spacing:0;text-transform:uppercase;margin:0 0 8px}h1,h2,h3,h4,h5,h6{font-family:Alegreya,"Iowan Old Style",Palatino,Georgia,serif;line-height:1.2;margin:1.5em 0 .45em;font-weight:700}h1{font-size:2.25rem;margin:0 0 .35em}h2{font-size:1.65rem}h3{font-size:1.35rem}.uuid{font:13px/1.4 "Fira Code","Fira Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:var(--muted);overflow-wrap:anywhere;margin:0}a{color:var(--accent);text-decoration-thickness:.08em;text-underline-offset:.16em}p{margin:0 0 1em}ul{padding-left:1.45em}strong{font-weight:700}em{font-style:italic}u{text-underline-offset:.12em}del{color:var(--muted)}table{width:100%;border-collapse:collapse;margin:1.2em 0;font-family:"Alegreya Sans",ui-sans-serif,system-ui,sans-serif;font-size:.95em}td,th{border:1px solid var(--border);padding:.45rem .6rem;vertical-align:top}tr:nth-child(even){background:color-mix(in srgb,var(--surface),var(--border) 15%)}pre{overflow:auto;background:var(--code);border:1px solid var(--border);border-radius:6px;padding:1rem;font:14px/1.55 "Fira Code","Fira Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}code{font-family:"Fira Code","Fira Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-variant-ligatures:contextual}.inline-code{background:var(--inline-code);border:1px solid var(--border);border-radius:4px;font-size:.86em;padding:.05rem .28rem}.code-orange{color:var(--orange-code);font-weight:600}figure{margin:1.2em 0}.code figcaption{font:12px/1.4 "Alegreya Sans",ui-sans-serif,system-ui,sans-serif;color:var(--muted);margin-bottom:.35rem}img{max-width:100%;height:auto;border:1px solid var(--border);border-radius:6px;background:var(--surface)}figcaption{color:var(--muted);font-size:.9em}.todo{font-size:.75em;border:1px solid var(--border);border-radius:4px;padding:.08rem .35rem;color:var(--muted);vertical-align:.12em}.math{font-family:"Cambria Math","STIX Two Math","Times New Roman",serif;background:var(--mark);border-radius:4px;padding:.08rem .28rem}.math-error{color:#b42318}.math-display{margin:1.35em 0;overflow-x:auto;text-align:center}.katex{font:normal 1.08em KaTeX_Main,"Cambria Math","STIX Two Math","Times New Roman",serif;line-height:1.2;text-indent:0;text-rendering:auto}.katex-display{display:block;text-align:center}.katex .katex-mathml{display:inline}.katex .katex-html{clip:rect(1px,1px,1px,1px);border:0;height:1px;overflow:hidden;padding:0;position:absolute;width:1px}.katex .base{display:inline-block}.katex .strut{display:inline-block}.katex .mord,.katex .mop,.katex .mbin,.katex .mrel,.katex .mopen,.katex .mclose,.katex .mpunct,.katex .minner{display:inline-block}.katex .mspace{display:inline-block}.katex .vlist-t{display:inline-table;table-layout:fixed}.katex .vlist-r{display:table-row}.katex .vlist{display:table-cell;vertical-align:bottom;position:relative}.katex .vlist>span{display:block;height:0;position:relative}.katex .vlist-s{display:table-cell;vertical-align:bottom;font-size:1px;width:2px;min-width:2px}.katex .sqrt>.root{margin-left:.27777778em;margin-right:-.55555556em}.katex .sqrt>.sqrt-sign{display:inline-block}.katex .frac-line{border-bottom-style:solid;display:block;width:100%}.katex .mfrac .frac-line{border-bottom-width:.04em}.katex .mfrac>span>span{text-align:center}.katex .msupsub{text-align:left}.katex .mfrac,.katex .msupsub,.katex .munder,.katex .mover,.katex .munderover{display:inline-block}.katex .mord.text{font-family:Alegreya,"Iowan Old Style",Palatino,Georgia,serif}.katex .mathnormal{font-style:italic}.katex .mathbf{font-weight:700}.katex .mathrm{font-style:normal}.katex .mspace.negativethinspace{margin-left:-.16666667em}"#
    .to_string();
    css.push_str(&syntect_css());
    css
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

[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Beta]]
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
        assert!(html.contains("<table>"));
        assert!(html.contains("class=\"katex\""));
        assert!(html.contains("<math"));
        assert!(html.contains("<code class=\"syn-code\">"));
        assert!(html.contains("syn-"));
        assert!(html.contains("main"));
        assert!(html.contains("<img src=\"/asset?note=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa&amp;"));
    }

    #[test]
    fn percent_codec_round_trips_url_components() {
        let value = "dir/name with spaces.png";
        assert_eq!(percent_decode(&percent_encode(value)), value);
    }

    #[test]
    fn renders_org_inline_formatting() {
        let html =
            render_formatted_text("*bold* /italic/ _under_ +gone+ =literal <tag>= ~orange~ $x^2$");

        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<u>under</u>"));
        assert!(html.contains("<del>gone</del>"));
        assert!(html.contains("<code class=\"inline-code\">literal &lt;tag&gt;</code>"));
        assert!(html.contains("<code class=\"inline-code code-orange\">orange</code>"));
        assert!(html.contains("class=\"katex\""));
        assert!(html.contains("<math"));
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
        assert!(css.contains("position:absolute"));
        assert!(css.contains("clip:rect(1px,1px,1px,1px)"));
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
