use super::highlight::highlight_code;
use super::inline::{
    escape_html, render_display_math, render_formatted_text, render_inline, render_standalone_image,
};
use super::page::{heading_anchor, render_heading_dates, render_heading_tags};
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node};
use crate::parser::{DEADLINE_RE, HEADING_RE, Heading, SCHEDULED_RE, parse_note};
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::sync::LazyLock;

pub(super) fn render_org_body(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
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
        if append_list_continuation(&mut html, &list_stack, line, graph, config, node) {
            pending_caption = None;
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

fn is_planning_line(trimmed: &str) -> bool {
    SCHEDULED_RE.is_match(trimmed) || DEADLINE_RE.is_match(trimmed)
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
    OrderedAlpha,
    OrderedNumber,
    Unordered,
}

impl ListKind {
    fn tag(self) -> &'static str {
        match self {
            Self::OrderedAlpha | Self::OrderedNumber => "ol",
            Self::Unordered => "ul",
        }
    }

    fn open_tag(self) -> &'static str {
        match self {
            Self::OrderedAlpha => r#"<ol type="a">"#,
            Self::OrderedNumber => "<ol>",
            Self::Unordered => "<ul>",
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
        let _ = writeln!(html, "{}", item.kind.open_tag());
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

fn append_list_continuation(
    html: &mut String,
    stack: &[ListFrame],
    line: &str,
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
) -> bool {
    let Some(frame) = stack.last().filter(|frame| frame.open_item) else {
        return false;
    };
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    if indent <= frame.indent {
        return false;
    }
    let text = line.trim();
    if text.is_empty() {
        return false;
    }
    html.push(' ');
    html.push_str(&render_inline(graph, config, node, text));
    true
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
    static ORDERED_NUMBER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d+[\.)]\s+(.+)$").unwrap());
    if let Some(text) = ORDERED_NUMBER_RE
        .captures(trimmed)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
    {
        return Some(ListItem {
            indent,
            kind: ListKind::OrderedNumber,
            text,
            checked: is_checked_item(text),
        });
    }
    static ORDERED_ALPHA_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[A-Za-z][\.)]\s+(.+)$").unwrap());
    ORDERED_ALPHA_RE
        .captures(trimmed)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .map(|text| ListItem {
            indent,
            kind: ListKind::OrderedAlpha,
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

pub(super) fn is_configured_todo_state(config: &ResolvedConfig, state: &str) -> bool {
    !state.is_empty()
        && (config
            .open_todo_states()
            .iter()
            .any(|open| open.eq_ignore_ascii_case(state))
            || is_closed_todo_state(config, state))
}

pub(super) fn heading_title_with_unconfigured_todo(
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
