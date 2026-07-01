use super::OrgRenderContext;
use regex::Regex;
use std::fmt::Write as FmtWrite;
use std::sync::LazyLock;

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

pub(super) struct ListFrame {
    kind: ListKind,
    indent: usize,
    open_item: bool,
}

pub(super) fn render_list_item(
    html: &mut String,
    stack: &mut Vec<ListFrame>,
    item: ListItem<'_>,
    context: &OrgRenderContext<'_>,
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
    html.push_str(&context.render_inline(item.text));
}

pub(super) fn append_list_continuation(
    html: &mut String,
    stack: &[ListFrame],
    line: &str,
    context: &OrgRenderContext<'_>,
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
    html.push_str(&context.render_inline(text));
    true
}

pub(super) fn close_lists(html: &mut String, stack: &mut Vec<ListFrame>) {
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

pub(super) struct ListItem<'a> {
    indent: usize,
    kind: ListKind,
    text: &'a str,
    checked: bool,
}

pub(super) fn list_item(line: &str) -> Option<ListItem<'_>> {
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
