use super::assets;
use super::page::heading_anchor;
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node, resolve_file_link_path};
use crate::parser::LINK_RE;
use regex::Regex;
use std::sync::LazyLock;

static PLAIN_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>"']+"#).expect("plain URL regex is valid"));

pub(super) fn render_inline(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    text: &str,
) -> String {
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
        let (href_uuid, anchor, preview_uuid) = if let Some(location) = graph.heading_location(uuid)
        {
            let preview_uuid = graph
                .resolve_target(uuid)
                .map(|node| node.uuid.to_string())
                .unwrap_or_else(|_| uuid.to_string());
            (
                location.primary_uuid.to_string(),
                Some(heading_anchor(location.line_number)),
                preview_uuid,
            )
        } else {
            let resolved_uuid = graph
                .resolve_target(uuid)
                .map(|node| node.uuid.to_string())
                .unwrap_or_else(|_| uuid.to_string());
            (resolved_uuid.clone(), None, resolved_uuid)
        };
        let anchor = anchor
            .map(|anchor| format!("#{}", percent_encode(&anchor)))
            .unwrap_or_default();
        return format!(
            "<a href=\"/?id={}{}\" data-preview-id=\"{}\">{}</a>",
            percent_encode(&href_uuid),
            anchor,
            escape_html(&preview_uuid),
            render_formatted_text(label)
        );
    }
    if let Some(path) = target.strip_prefix("file:") {
        let resolved = resolve_file_link_path(path, &node.path, config.resolved_db_root());
        let href = asset_href(&node.uuid, assets::AssetKind::File, path);
        if assets::is_image_path(&resolved) {
            return format!(
                "<figure><img src=\"{href}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
                escape_html(label),
                render_formatted_text(label)
            );
        }
        return format!("<a href=\"{href}\">{}</a>", render_formatted_text(label));
    }
    if let Some(path) = target.strip_prefix("attachment:") {
        let resolved =
            assets::resolve_existing_attachment(config.resolved_db_root(), &node.uuid, path);
        let href = asset_href(&node.uuid, assets::AssetKind::Attachment, path);
        if assets::is_image_path(&resolved) {
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

pub(super) fn render_standalone_image(
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
        if assets::is_image_path(&resolved) {
            return Some(render_image_figure(
                &asset_href(&node.uuid, assets::AssetKind::File, path),
                caption.or(desc).unwrap_or(target),
            ));
        }
    }
    if let Some(path) = target.strip_prefix("attachment:") {
        let resolved =
            assets::resolve_existing_attachment(config.resolved_db_root(), &node.uuid, path);
        if assets::is_image_path(&resolved) {
            return Some(render_image_figure(
                &asset_href(&node.uuid, assets::AssetKind::Attachment, path),
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

fn asset_href(note_uuid: &str, kind: assets::AssetKind, target: &str) -> String {
    format!(
        "/asset?note={}&amp;kind={}&amp;target={}",
        percent_encode(note_uuid),
        percent_encode(kind.as_str()),
        percent_encode(target)
    )
}

pub(super) fn render_formatted_text(text: &str) -> String {
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

pub(super) fn render_formatted_text_with_plain_links(text: &str) -> String {
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

pub(super) fn render_display_math(input: &str) -> String {
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

pub(super) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn percent_encode(text: &str) -> String {
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

pub(super) fn percent_decode(text: &str) -> String {
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
