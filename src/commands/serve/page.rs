use super::inline::{escape_html, percent_encode, render_formatted_text};
use super::org_html::{
    heading_title_with_unconfigured_todo, is_configured_todo_state, render_org_body,
};
use super::{page_css, page_js};
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node};
use crate::org_edit::parsed_heading_subtree_end_index;
use crate::parser::{HEADING_RE, Heading, parse_note, strip_org_links};
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;

pub(super) fn render_note_html(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
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
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
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

pub(super) fn render_preview_html(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
    let body = if let Some(block) = heading_preview_content(graph, node, content) {
        render_org_body(graph, config, node, &block)
    } else {
        render_org_body(graph, config, node, content)
    };
    let tags = render_tag_list("Note tags", &node.filetags, "note-tags");
    let title = escape_html(&node.title);
    format!(
        "<section class=\"note-preview-content\" data-preview-note=\"{}\">\n<header class=\"note-preview-header\">\n<p class=\"eyebrow\">pkms note</p>\n<h1>{title}</h1>\n{tags}</header>\n<article class=\"note-body note-preview-body\">\n{body}</article>\n</section>\n",
        escape_html(&node.uuid)
    )
}

fn heading_preview_content(graph: &Graph, node: &Node, content: &str) -> Option<String> {
    graph.primary_uuid_for_heading(&node.uuid)?;

    let parsed = parse_note(content);
    let headings = parsed.headings;
    let target_idx = headings
        .iter()
        .position(|heading| heading.uuid.as_deref() == Some(node.uuid.as_str()))?;
    let target = &headings[target_idx];
    let start = target.line_number.checked_sub(1)?;
    let lines: Vec<&str> = content.lines().collect();
    let end =
        parsed_heading_subtree_end_index(&headings, target.line_number, target.level, lines.len());
    if start >= lines.len() || end <= start {
        return None;
    }
    let mut block = lines[start..end].join("\n");
    block.push('\n');
    Some(block)
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
        "<details class=\"side-panel contents-panel\">\n<summary>Contents</summary>\n",
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

pub(super) fn render_heading_tags(tags: &[String]) -> String {
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

pub(super) fn render_heading_dates(heading: &Heading) -> String {
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

fn render_backlinks_panel(graph: &Graph, node: &Node) -> String {
    let mut incoming: BTreeMap<(String, String), &Node> = BTreeMap::new();
    if let Some(backlink_uuids) = graph.backlinks.get(&node.uuid) {
        for uuid in backlink_uuids {
            if let Some(source) = graph.nodes.get(uuid) {
                incoming.insert(
                    (source.title.to_ascii_lowercase(), source.uuid.to_string()),
                    source,
                );
            }
        }
    }

    let mut html = String::from(
        "<details class=\"side-panel backlinks-panel\">\n<summary>Backlinks</summary>\n",
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

pub(super) fn heading_anchor(line_number: usize) -> String {
    format!("h-{line_number}")
}
