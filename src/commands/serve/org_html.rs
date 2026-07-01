use super::inline::{escape_html, render_display_math, render_inline, render_standalone_image};
use super::page::{heading_anchor, render_heading_dates, render_heading_tags};
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node};
use crate::parser::{DEADLINE_RE, HEADING_RE, Heading, SCHEDULED_RE, parse_note};
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;

mod blocks;
mod lists;

use blocks::{read_org_block, render_org_block, render_table};
use lists::{ListFrame, append_list_continuation, close_lists, list_item, render_list_item};

pub(super) fn render_org_body(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
    let context = OrgRenderContext {
        graph,
        config,
        node,
    };
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
            flush_paragraph(&mut html, &mut paragraph, &context);
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
            flush_paragraph(&mut html, &mut paragraph, &context);
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
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            i += 1;
            continue;
        }
        if lower.starts_with("#+caption:") {
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            (pending_caption, i) = read_caption(&lines, i);
            continue;
        }
        if let Some((block, next_i)) = read_org_block(&lines, i) {
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            let caption = pending_caption.take();
            html.push_str(&render_org_block(&context, &block, caption.as_deref()));
            i = next_i;
            continue;
        }
        if trimmed == r"\[" {
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            let (formula, next_i) = read_display_math(&lines, i);
            html.push_str(&render_display_math(formula.trim()));
            i = next_i;
            continue;
        }
        if trimmed.starts_with('|') {
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            let (table_lines, next_i) = read_table_lines(&lines, i);
            html.push_str(&render_table(&table_lines, &context));
            i = next_i;
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
            flush_paragraph(&mut html, &mut paragraph, &context);
            pending_caption = None;
            render_list_item(&mut html, &mut list_stack, item, &context);
            i += 1;
            continue;
        }
        if append_list_continuation(&mut html, &list_stack, line, &context) {
            pending_caption = None;
            i += 1;
            continue;
        }
        if let Some(cap) = HEADING_RE.captures(line) {
            flush_paragraph(&mut html, &mut paragraph, &context);
            close_lists(&mut html, &mut list_stack);
            pending_caption = None;
            html.push_str(&render_heading_line(
                &context,
                &headings_by_line,
                i + 1,
                &cap,
            ));
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

    flush_paragraph(&mut html, &mut paragraph, &context);
    close_lists(&mut html, &mut list_stack);
    html
}

pub(super) struct OrgRenderContext<'a> {
    graph: &'a Graph,
    config: &'a ResolvedConfig,
    node: &'a Node,
}

impl OrgRenderContext<'_> {
    pub(super) fn render_inline(&self, text: &str) -> String {
        render_inline(self.graph, self.config, self.node, text)
    }
}

fn is_planning_line(trimmed: &str) -> bool {
    SCHEDULED_RE.is_match(trimmed) || DEADLINE_RE.is_match(trimmed)
}

fn read_caption(lines: &[&str], start: usize) -> (Option<String>, usize) {
    let mut caption = lines[start]
        .trim()
        .split_once(':')
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_default();
    let mut i = start + 1;
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
    ((!caption.is_empty()).then_some(caption), i)
}

fn read_display_math(lines: &[&str], start: usize) -> (String, usize) {
    let mut formula = String::new();
    let mut i = start + 1;
    while i < lines.len() && lines[i].trim() != r"\]" {
        formula.push_str(lines[i].trim());
        formula.push('\n');
        i += 1;
    }
    if i < lines.len() {
        i += 1;
    }
    (formula, i)
}

fn read_table_lines<'a>(lines: &[&'a str], start: usize) -> (Vec<&'a str>, usize) {
    let mut table_lines = Vec::new();
    let mut i = start;
    while i < lines.len() && lines[i].trim().starts_with('|') {
        table_lines.push(lines[i].trim());
        i += 1;
    }
    (table_lines, i)
}

fn render_heading_line(
    context: &OrgRenderContext<'_>,
    headings_by_line: &BTreeMap<usize, &Heading>,
    line_number: usize,
    cap: &regex::Captures<'_>,
) -> String {
    let level = cap[1].len().saturating_add(1).min(6);
    let raw_todo = cap.get(2).map(|m| m.as_str());
    let todo = raw_todo
        .filter(|state| is_configured_todo_state(context.config, state))
        .unwrap_or_default();
    let title = heading_title_with_unconfigured_todo(
        raw_todo.filter(|_| todo.is_empty()),
        cap.get(3).map(|m| m.as_str()),
        cap.get(4).map_or("", |m| m.as_str()),
    );
    let heading = headings_by_line.get(&line_number).copied();
    let anchor = heading_anchor(line_number);
    let mut html = if is_closed_todo_state(context.config, todo) {
        format!(
            "<h{level} id=\"{}\" class=\"closed-heading\">",
            escape_html(&anchor)
        )
    } else {
        format!("<h{level} id=\"{}\">", escape_html(&anchor))
    };
    if !todo.is_empty() {
        html.push_str(&format!(
            "<span class=\"todo\">{}</span> ",
            escape_html(todo)
        ));
    }
    html.push_str(&context.render_inline(&title));
    if let Some(heading) = heading {
        html.push_str(&render_heading_tags(&heading.tags));
    }
    html.push_str(&format!("</h{level}>\n"));
    if let Some(heading) = heading {
        html.push_str(&render_heading_dates(heading));
    }
    html
}

fn flush_paragraph(html: &mut String, paragraph: &mut Vec<&str>, context: &OrgRenderContext<'_>) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join("\n");
    html.push_str("<p>");
    html.push_str(&context.render_inline(&text));
    html.push_str("</p>\n");
    paragraph.clear();
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
