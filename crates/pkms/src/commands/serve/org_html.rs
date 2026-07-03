use super::inline::{escape_html, render_display_math, render_inline, render_standalone_image};
use super::page::{heading_anchor, render_heading_dates, render_heading_tags};
use crate::config::ResolvedConfig;
use pkms_org::graph::{Graph, Node};
use pkms_org::parser::{DEADLINE_RE, HEADING_RE, Heading, SCHEDULED_RE, parse_note};
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;

mod blocks;
mod lists;

use blocks::{read_org_block, render_org_block, render_table};
use lists::{
    ListFrame, append_list_continuation as append_list_continuation_html, close_lists, list_item,
    render_list_item as render_list_item_html,
};

pub(super) fn render_org_body(
    graph: &Graph,
    config: &ResolvedConfig,
    node: &Node,
    content: &str,
) -> String {
    OrgBodyRenderer::new(graph, config, node, content).render()
}

struct OrgBodyRenderer<'a> {
    context: OrgRenderContext<'a>,
    lines: Vec<&'a str>,
    headings_by_line: BTreeMap<usize, Heading>,
    html: String,
    paragraph: Vec<&'a str>,
    list_stack: Vec<ListFrame>,
    pending_caption: Option<String>,
}

impl<'a> OrgBodyRenderer<'a> {
    fn new(graph: &'a Graph, config: &'a ResolvedConfig, node: &'a Node, content: &'a str) -> Self {
        let parsed = parse_note(content);
        let headings_by_line = parsed
            .headings
            .into_iter()
            .map(|heading| (heading.line_number, heading))
            .collect();
        Self {
            context: OrgRenderContext {
                graph,
                config,
                node,
            },
            lines: content.lines().collect(),
            headings_by_line,
            html: String::new(),
            paragraph: Vec::new(),
            list_stack: Vec::new(),
            pending_caption: None,
        }
    }

    fn render(mut self) -> String {
        let mut i = 0;
        while i < self.lines.len() {
            i = self.render_line(i);
        }
        self.end_flow();
        self.html
    }

    fn render_line(&mut self, i: usize) -> usize {
        let line = self.lines[i];
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if trimmed == ":PROPERTIES:" {
            return self.skip_properties(i);
        }
        if trimmed.is_empty() {
            self.end_flow_and_drop_caption();
            return i + 1;
        }
        if lower.starts_with("#+title:") || lower.starts_with("#+filetags:") {
            return i + 1;
        }
        if is_planning_line(trimmed) {
            self.end_flow_and_drop_caption();
            return i + 1;
        }
        if lower.starts_with("#+caption:") {
            return self.read_caption(i);
        }
        if let Some(next_i) = self.render_block(i) {
            return next_i;
        }
        if trimmed == r"\[" {
            return self.render_display_math(i);
        }
        if trimmed.starts_with('|') {
            return self.render_table(i);
        }
        if self.render_standalone_image(trimmed) {
            return i + 1;
        }
        if self.render_list_item(line) {
            return i + 1;
        }
        if self.append_list_continuation(line) {
            return i + 1;
        }
        if let Some(cap) = HEADING_RE.captures(line) {
            self.render_heading(i + 1, &cap);
            return i + 1;
        }
        if trimmed.starts_with("#+") {
            self.pending_caption = None;
            return i + 1;
        }

        self.push_paragraph_line(line);
        i + 1
    }

    fn skip_properties(&mut self, start: usize) -> usize {
        self.end_flow();
        let mut i = start + 1;
        while i < self.lines.len() {
            if self.lines[i].trim() == ":END:" {
                return i + 1;
            }
            i += 1;
        }
        i
    }

    fn read_caption(&mut self, i: usize) -> usize {
        self.end_flow();
        let (caption, next_i) = read_caption(&self.lines, i);
        self.pending_caption = caption;
        next_i
    }

    fn render_block(&mut self, i: usize) -> Option<usize> {
        let (block, next_i) = read_org_block(&self.lines, i)?;
        self.end_flow();
        let caption = self.pending_caption.take();
        self.html
            .push_str(&render_org_block(&self.context, &block, caption.as_deref()));
        Some(next_i)
    }

    fn render_display_math(&mut self, i: usize) -> usize {
        self.end_flow_and_drop_caption();
        let (formula, next_i) = read_display_math(&self.lines, i);
        self.html.push_str(&render_display_math(formula.trim()));
        next_i
    }

    fn render_table(&mut self, i: usize) -> usize {
        self.end_flow_and_drop_caption();
        let (table_lines, next_i) = read_table_lines(&self.lines, i);
        self.html
            .push_str(&render_table(&table_lines, &self.context));
        next_i
    }

    fn render_standalone_image(&mut self, trimmed: &str) -> bool {
        let Some(caption) = self.pending_caption.take() else {
            return false;
        };
        let Some(figure) =
            render_standalone_image(self.context.config, self.context.node, trimmed, &caption)
        else {
            return false;
        };
        self.html.push_str(&figure);
        true
    }

    fn render_list_item(&mut self, line: &'a str) -> bool {
        let Some(item) = list_item(line) else {
            return false;
        };
        self.flush_paragraph();
        self.pending_caption = None;
        render_list_item_html(&mut self.html, &mut self.list_stack, item, &self.context);
        true
    }

    fn append_list_continuation(&mut self, line: &str) -> bool {
        if !append_list_continuation_html(&mut self.html, &self.list_stack, line, &self.context) {
            return false;
        }
        self.pending_caption = None;
        true
    }

    fn render_heading(&mut self, line_number: usize, cap: &regex::Captures<'_>) {
        self.end_flow_and_drop_caption();
        self.html.push_str(&render_heading_line(
            &self.context,
            &self.headings_by_line,
            line_number,
            cap,
        ));
    }

    fn push_paragraph_line(&mut self, line: &'a str) {
        self.pending_caption = None;
        close_lists(&mut self.html, &mut self.list_stack);
        self.paragraph.push(line);
    }

    fn end_flow_and_drop_caption(&mut self) {
        self.end_flow();
        self.pending_caption = None;
    }

    fn end_flow(&mut self) {
        self.flush_paragraph();
        close_lists(&mut self.html, &mut self.list_stack);
    }

    fn flush_paragraph(&mut self) {
        if self.paragraph.is_empty() {
            return;
        }
        let text = self.paragraph.join("\n");
        self.html.push_str("<p>");
        self.html.push_str(&self.context.render_inline(&text));
        self.html.push_str("</p>\n");
        self.paragraph.clear();
    }
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
    headings_by_line: &BTreeMap<usize, Heading>,
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
    let heading = headings_by_line.get(&line_number);
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
