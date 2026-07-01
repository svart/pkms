//! Org-mode note parser.
//!
//! [`ParsedNote`] is the result of parsing a single `.org` file. It extracts UUIDs (from
//! `:ID:` properties), `#+title`, `#+filetags`, `ROAM_ALIASES`, `ROAM_REFS`, `CATEGORY`,
//! SCHEDULED/DEADLINE timestamps, org-mode links (`[[id:...]]`, `[[file:...]]`, `[[url:...]]`),
//! and headings with their TODO states, priorities, tags, and line numbers.

use crate::domain::{LinkTarget, NoteId};
use crate::tasks::model::{TaskPriority, TaskState};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub uuids: Vec<NoteId>,
    pub title: Option<String>,
    pub filetags: Vec<String>,
    pub project: Option<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub roam_refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNoteSummary {
    pub uuids: Vec<NoteId>,
    pub title: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub has_todos: bool,
}

impl ParsedNote {
    pub fn empty() -> Self {
        ParsedNote {
            uuids: vec![],
            title: None,
            filetags: vec![],
            project: None,
            categories: vec![],
            aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        }
    }

    pub fn heading_uuids(&self) -> Vec<NoteId> {
        self.headings
            .iter()
            .filter_map(|h| h.uuid.clone())
            .collect()
    }

    pub fn has_todo_headings(&self) -> bool {
        self.headings.iter().any(|h| {
            h.todo_state
                .as_ref()
                .is_some_and(|s| s.as_str().chars().all(|c| c.is_uppercase() || c == '-'))
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Link {
    Internal(NoteId),
    File(LinkTarget),
    Url(LinkTarget),
    Attachment(LinkTarget),
}

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: usize,
    pub title: String,
    pub todo_state: Option<TaskState>,
    pub tags: Vec<String>,
    pub uuid: Option<NoteId>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub priority: Option<TaskPriority>,
    pub project: Option<String>,
    pub line_number: usize,
    pub outgoing: Vec<Link>,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyKey {
    Id,
    Category,
    Project,
    RoamAliases,
    RoamRefs,
    Other,
}

impl PropertyKey {
    fn parse(key: &str) -> Self {
        match key.to_ascii_uppercase().as_str() {
            "ID" => PropertyKey::Id,
            "CATEGORY" => PropertyKey::Category,
            "PROJECT" => PropertyKey::Project,
            "ROAM_ALIASES" => PropertyKey::RoamAliases,
            "ROAM_REFS" => PropertyKey::RoamRefs,
            _ => PropertyKey::Other,
        }
    }
}

pub(crate) static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]]+?)(?:\]\[([^\]]*))?\]\]").unwrap());

pub(crate) static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\*+)\s+(?:([A-Z][A-Z-]*)\s+)?(?:\[#([A-C])\]\s+)?(.*?)(?:\s+:(\w+(?::\w+)*):)?\s*$",
    )
    .unwrap()
});

pub(crate) static SCHEDULED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"SCHEDULED:\s*(<[^>]+>(?:--<[^>]+>)?)").unwrap());

pub(crate) static DEADLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"DEADLINE:\s*(<[^>]+>(?:--<[^>]+>)?)").unwrap());

pub(crate) static DAILY_FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})\.org$").unwrap());

pub(crate) static TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+title:\s*(.*)$").unwrap());

pub(crate) static FILETAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+filetags:\s*(.+)$").unwrap());

pub(crate) static ID_PROPERTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i):ID:\s+([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})")
        .unwrap()
});

static CATEGORY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i):CATEGORY:\s+(.+)").unwrap());

static ALIASES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i):ROAM_ALIASES:\s+(.*)").unwrap());

pub(crate) static UUID_FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$").unwrap()
});

struct ParseContext {
    uuids: Vec<NoteId>,
    title: Option<String>,
    filetags: Vec<String>,
    project: Option<String>,
    categories: Vec<String>,
    aliases: Vec<String>,
    roam_refs: Vec<String>,
    outgoing: Vec<Link>,
    headings: Vec<Heading>,
    in_properties: bool,
    in_src_block: bool,
    current_heading_idx: Option<usize>,
    just_saw_heading: bool,
    heading_stack: Vec<usize>,
}

impl ParseContext {
    fn new() -> Self {
        ParseContext {
            uuids: Vec::new(),
            title: None,
            filetags: Vec::new(),
            project: None,
            categories: Vec::new(),
            aliases: Vec::new(),
            roam_refs: Vec::new(),
            outgoing: Vec::new(),
            headings: Vec::new(),
            in_properties: false,
            in_src_block: false,
            current_heading_idx: None,
            just_saw_heading: false,
            heading_stack: Vec::new(),
        }
    }

    fn finalize(self) -> ParsedNote {
        ParsedNote {
            uuids: self.uuids,
            title: self.title,
            filetags: self.filetags,
            project: self.project,
            categories: self.categories,
            aliases: self.aliases,
            roam_refs: self.roam_refs,
            outgoing: self.outgoing,
            headings: self.headings,
        }
    }

    fn process_line(&mut self, line_idx: usize, line: &str) {
        let trimmed = line.trim();

        if self.in_src_block {
            if trimmed.eq_ignore_ascii_case("#+end_src") {
                self.in_src_block = false;
            }
            return;
        }

        if starts_with_ignore_ascii_case(trimmed, "#+begin_src") {
            self.in_src_block = true;
            return;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            self.in_properties = true;
            return;
        }
        if trimmed.eq_ignore_ascii_case(":END:") {
            self.in_properties = false;
            return;
        }

        if self.in_properties {
            self.handle_property_line(trimmed);
        } else {
            self.handle_body_line(line_idx, line, trimmed);
        }
    }

    fn handle_property_line(&mut self, trimmed: &str) {
        let Some((key, value)) = parse_property(trimmed) else {
            return;
        };
        match key {
            PropertyKey::Id => {
                if let Some(idx) = self.current_heading_idx {
                    self.headings[idx].uuid = Some(NoteId::new(value));
                } else {
                    self.uuids.push(NoteId::new(value));
                }
            }
            PropertyKey::Category => self.categories.push(value.to_string()),
            PropertyKey::Project => {
                if let Some(idx) = self.current_heading_idx {
                    self.headings[idx].project = Some(value.to_string());
                } else {
                    self.project = Some(value.to_string());
                }
            }
            PropertyKey::RoamAliases => {
                self.aliases = parse_property_words(value);
            }
            PropertyKey::RoamRefs => {
                self.roam_refs = value
                    .split_whitespace()
                    .map(std::string::ToString::to_string)
                    .collect();
            }
            PropertyKey::Other => {}
        }
    }

    fn handle_body_line(&mut self, line_idx: usize, line: &str, trimmed: &str) {
        self.try_extract_title(line);
        self.try_extract_filetags(line);

        if self.try_parse_heading(line_idx, line) {
            return;
        }

        self.try_parse_scheduled_deadline(trimmed);
        self.extract_links(line);
    }

    fn try_extract_title(&mut self, line: &str) {
        if self.title.is_some() {
            return;
        }
        if let Some(cap) = TITLE_RE.captures(line) {
            self.title = Some(cap[1].trim().to_string());
        }
    }

    fn try_extract_filetags(&mut self, line: &str) {
        let Some(cap) = FILETAGS_RE.captures(line) else {
            return;
        };
        let tags_str = cap.get(1).map_or("", |m| m.as_str());
        for tag in tags_str.split(':') {
            let tag = tag.trim();
            if !tag.is_empty() {
                self.filetags.push(tag.to_string());
            }
        }
    }

    fn try_parse_heading(&mut self, line_idx: usize, line: &str) -> bool {
        let Some(cap) = HEADING_RE.captures(line) else {
            return false;
        };
        if cap[1].len() <= 1 && cap.get(4).is_none_or(|m| m.as_str().is_empty()) {
            return false;
        }

        let level = cap[1].len();
        while let Some(&top_idx) = self.heading_stack.last() {
            if self.headings[top_idx].level >= level {
                self.heading_stack.pop();
            } else {
                break;
            }
        }

        let todo_state = cap.get(2).map(|m| TaskState::new(m.as_str()));
        let priority = cap
            .get(3)
            .and_then(|m| m.as_str().chars().next())
            .and_then(TaskPriority::from_char);
        let heading_title = cap.get(4).map_or("", |m| m.as_str()).to_string();
        let tags = cap
            .get(5)
            .map(|m| {
                m.as_str()
                    .split(':')
                    .filter(|t| !t.is_empty())
                    .map(std::string::ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();
        self.headings.push(Heading {
            level,
            title: heading_title,
            todo_state,
            tags,
            uuid: None,
            scheduled: None,
            deadline: None,
            priority,
            project: None,
            line_number: line_idx + 1,
            outgoing: vec![],
            raw: line.to_string(),
        });
        self.heading_stack.push(self.headings.len() - 1);
        self.current_heading_idx = Some(self.headings.len() - 1);
        self.just_saw_heading = true;

        for cap in LINK_RE.captures_iter(line) {
            let link_target = cap[1].to_string();
            if let Some(link) = parse_link(&link_target) {
                if let Some(&idx) = self.heading_stack.last() {
                    self.headings[idx].outgoing.push(link);
                } else {
                    self.outgoing.push(link);
                }
            }
        }

        true
    }

    fn try_parse_scheduled_deadline(&mut self, trimmed: &str) {
        if !self.just_saw_heading || trimmed.is_empty() {
            return;
        }
        if let Some(cap) = SCHEDULED_RE.captures(trimmed)
            && let Some(idx) = self.current_heading_idx
        {
            self.headings[idx].scheduled = cap.get(1).map(|m| m.as_str().to_string());
        }
        if let Some(cap) = DEADLINE_RE.captures(trimmed)
            && let Some(idx) = self.current_heading_idx
        {
            self.headings[idx].deadline = cap.get(1).map(|m| m.as_str().to_string());
        }
        self.just_saw_heading = false;
    }

    fn extract_links(&mut self, line: &str) {
        for cap in LINK_RE.captures_iter(line) {
            let link_target = cap[1].to_string();
            if let Some(link) = parse_link(&link_target) {
                if let Some(&idx) = self.heading_stack.last() {
                    self.headings[idx].outgoing.push(link);
                } else {
                    self.outgoing.push(link);
                }
            }
        }
    }
}

pub fn parse_note(content: &str) -> ParsedNote {
    let mut ctx = ParseContext::new();
    for (line_idx, line) in content.lines().enumerate() {
        ctx.process_line(line_idx, line);
    }
    ctx.finalize()
}

pub fn parse_note_summary(content: &str) -> ParsedNoteSummary {
    let uuids = ID_PROPERTY_RE
        .captures_iter(content)
        .filter_map(|c| c.get(1))
        .map(|m| NoteId::new(m.as_str()))
        .collect();

    let title = TITLE_RE
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();

    let filetags = FILETAGS_RE
        .captures(content)
        .map(|c| {
            c.get(1)
                .map_or("", |m| m.as_str())
                .split(':')
                .filter(|t| !t.is_empty())
                .map(|t| t.trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    let aliases = ALIASES_RE
        .captures_iter(content)
        .last()
        .map(|c| parse_property_words(c.get(1).map_or("", |m| m.as_str())))
        .unwrap_or_default();

    let categories = CATEGORY_RE
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .collect();

    ParsedNoteSummary {
        uuids,
        title,
        filetags,
        categories,
        aliases,
        has_todos: parse_note(content).has_todo_headings(),
    }
}

pub fn strip_org_links(text: &str) -> String {
    LINK_RE
        .replace_all(text, |caps: &regex::Captures| {
            caps.get(2)
                .map(|m| m.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| caps.get(1).map(|m| m.as_str()))
                .unwrap_or("")
                .to_string()
        })
        .to_string()
}

pub fn find_daily_file_date(path: &std::path::Path) -> Option<NaiveDate> {
    let filename = path.file_name()?.to_str()?;
    let cap = DAILY_FILE_RE.captures(filename)?;
    NaiveDate::parse_from_str(cap.get(1)?.as_str(), "%Y-%m-%d").ok()
}

fn parse_property(line: &str) -> Option<(PropertyKey, &str)> {
    let line = line.trim();
    if line.starts_with(':')
        && let Some(end) = line[1..].find(':')
    {
        let key = &line[1..=end];
        let value = line[end + 2..].trim();
        if !key.is_empty() && !value.is_empty() {
            return Some((PropertyKey::parse(key), value));
        }
    }
    None
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn unquote_property_word(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn parse_property_words(value: &str) -> Vec<String> {
    shlex::split(value).unwrap_or_else(|| {
        value
            .split_whitespace()
            .map(unquote_property_word)
            .collect()
    })
}

/// Validate that all `#+filetags:` lines in content have the correct format.
/// Valid format: `:tag1:tag2:tag3:` — leading colon, trailing colon, tags separated
/// by single colons, no empty segments.
/// Tags may contain spaces (e.g. `:ai encyclopedia:test:` is valid).
/// Returns `(raw_value, description)` for each invalid line.
pub fn validate_filetags_format(content: &str) -> Vec<(String, String)> {
    let mut issues = Vec::new();
    for cap in FILETAGS_RE.captures_iter(content) {
        let value = cap.get(1).map_or("", |m| m.as_str());
        let trimmed = value.trim();

        let mut reasons = Vec::new();

        if !trimmed.starts_with(':') || !trimmed.ends_with(':') {
            reasons.push("must start and end with ':'".to_string());
        }
        if trimmed.contains("::") {
            reasons.push("must not have empty segments (::)".to_string());
        }
        let inner = if trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            ""
        };
        for segment in inner.split(':') {
            if segment.is_empty() || segment.chars().all(|c| c.is_whitespace()) {
                reasons.push(
                    "must not have empty or whitespace-only segments between colons".to_string(),
                );
                break;
            }
        }

        if !reasons.is_empty() {
            issues.push((value.to_string(), reasons.join("; ")));
        }
    }
    issues
}

fn parse_link(target: &str) -> Option<Link> {
    if let Some(rest) = target.strip_prefix("id:") {
        return Some(Link::Internal(NoteId::new(rest)));
    }
    if let Some(rest) = target.strip_prefix("file:") {
        return Some(Link::File(LinkTarget::new(rest)));
    }
    if target.starts_with("org:") {
        return Some(Link::File(LinkTarget::new(target)));
    }
    if let Some(rest) = target.strip_prefix("attachment:") {
        return Some(Link::Attachment(LinkTarget::new(rest)));
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(Link::Url(LinkTarget::new(target)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_parse_basic_note() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: test note

Some content here."#;
        let note = parse_note(content);
        assert_eq!(
            note.uuids.first().unwrap(),
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        );
        assert_eq!(note.title.unwrap(), "test note");
        assert!(note.filetags.is_empty());
        assert!(note.outgoing.is_empty());
    }

    #[test]
    fn test_parse_with_links() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:ROAM_ALIASES: Test Alias
:ROAM_REFS: https://example.com
:END:
#+title: test note
#+filetags: :book:tech:

[[id:deadbeef-dead-beef-dead-beef00000001][linked note]]
[[https://example.org][external]]
[[file:~/docs/manual.pdf][manual]]
[[attachment:image.jpg]]
"#;
        let note = parse_note(content);
        assert_eq!(note.filetags, vec!["book", "tech"]);
        assert_eq!(note.aliases, vec!["Test", "Alias"]);
        assert_eq!(note.roam_refs, vec!["https://example.com"]);
        assert_eq!(note.outgoing.len(), 4);
        assert!(matches!(note.outgoing[0], Link::Internal(_)));
    }

    #[test]
    fn test_parse_ignores_uppercase_source_blocks() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: source block note

#+BEGIN_SRC org
[[id:deadbeef-dead-beef-dead-beef00000001]]
* TODO Not a task
#+END_SRC
"#;
        let note = parse_note(content);
        assert!(note.outgoing.is_empty());
        assert!(note.headings.is_empty());
    }

    #[test]
    fn test_parse_uppercase_filetags_keyword() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+TITLE: uppercase keyword note
#+FILETAGS: :book:tech:
"#;
        let note = parse_note(content);
        assert_eq!(note.filetags, vec!["book", "tech"]);

        let summary = parse_note_summary(content);
        assert_eq!(summary.filetags, vec!["book", "tech"]);
    }

    #[test]
    fn test_parse_mixed_case_property_drawer_markers_and_keys() {
        let content = r#":properties:
:id:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:category: example
:roam_aliases: "Alias One"
:end:
#+title: mixed case drawer note
"#;
        let note = parse_note(content);
        assert_eq!(
            note.uuids.first().map(NoteId::as_str),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
        );
        assert_eq!(note.categories, vec!["example"]);
        assert_eq!(note.aliases, vec!["Alias One"]);
    }

    #[test]
    fn test_parse_quoted_aliases_with_spaces() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:ROAM_ALIASES: "Alias One" "Alias Two"
:END:
#+title: test note
"#;
        let note = parse_note(content);
        assert_eq!(note.aliases, vec!["Alias One", "Alias Two"]);

        let summary = parse_note_summary(content);
        assert_eq!(summary.aliases, vec!["Alias One", "Alias Two"]);
    }

    #[test]
    fn test_parse_headings() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: headings test

* TODO Section 1 :tag1:
Some text
** DONE Subsection :tag2:tag3:
*** Normal heading"#;
        let note = parse_note(content);
        assert_eq!(note.headings.len(), 3);
        assert_eq!(note.headings[0].title, "Section 1");
        assert_eq!(note.headings[0].level, 1);
        assert_eq!(note.headings[0].todo_state.as_deref(), Some("TODO"));
        assert_eq!(note.headings[0].tags, vec!["tag1"]);
        assert_eq!(note.headings[1].tags, vec!["tag2", "tag3"]);
        assert!(note.headings[2].tags.is_empty());
    }

    #[test]
    fn test_parse_heading_uuids() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: heading uuids

* Section 1
Text
** Subsection A
:PROPERTIES:
:ID:       sub-uuid-aaaa-0000-000000000001
:END:
* Section 2
:PROPERTIES:
:ID:       sec-uuid-bbbb-0000-000000000002
:END:
Some text
** Subsection B"#;
        let note = parse_note(content);
        assert_eq!(note.uuids.len(), 1);
        assert_eq!(note.uuids[0], "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
        assert_eq!(note.headings.len(), 4);
        assert!(note.headings[0].uuid.is_none());
        assert_eq!(
            note.headings[1].uuid.as_deref(),
            Some("sub-uuid-aaaa-0000-000000000001")
        );
        assert_eq!(
            note.headings[2].uuid.as_deref(),
            Some("sec-uuid-bbbb-0000-000000000002")
        );
        assert!(note.headings[3].uuid.is_none());
        assert_eq!(
            note.heading_uuids(),
            vec![
                "sub-uuid-aaaa-0000-000000000001",
                "sec-uuid-bbbb-0000-000000000002"
            ]
        );
    }

    #[test]
    fn test_parse_project_properties() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:PROJECT: Note Project
:END:
#+title: project properties

* TODO Note project task
* TODO Heading project task
:PROPERTIES:
:PROJECT: Heading Project
:END:
"#;
        let note = parse_note(content);
        assert_eq!(note.project.as_deref(), Some("Note Project"));
        assert_eq!(note.headings[0].project.as_deref(), None);
        assert_eq!(note.headings[1].project.as_deref(), Some("Heading Project"));
    }

    #[test]
    fn test_parse_category() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:CATEGORY: example
:END:
#+title: categorized note

Some content."#;
        let note = parse_note(content);
        assert_eq!(note.categories, vec!["example"]);
    }

    #[test]
    fn test_parse_multiple_uuids() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: multi uuid note

* Heading
:PROPERTIES:
:ID:       deadbeef-dead-beef-dead-beef00000001
:END:"#;
        let note = parse_note(content);
        assert_eq!(note.uuids.len(), 1);
        assert_eq!(note.uuids[0], "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
        assert_eq!(note.headings.len(), 1);
        assert_eq!(
            note.headings[0].uuid.as_deref(),
            Some("deadbeef-dead-beef-dead-beef00000001")
        );
        assert_eq!(
            note.heading_uuids(),
            vec!["deadbeef-dead-beef-dead-beef00000001"]
        );
    }

    #[test]
    fn test_validate_filetags_format_valid() {
        let content = "#+filetags: :tag1:tag2:tag3:\n";
        let issues = validate_filetags_format(content);
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
    }

    #[test]
    fn test_validate_filetags_format_spaces() {
        let content = "#+filetags: :tag1: :tag2:\n";
        let issues = validate_filetags_format(content);
        assert!(!issues.is_empty(), "expected space-related issue");
        assert!(issues[0].1.contains("whitespace-only"));
    }

    #[test]
    fn test_validate_filetags_multi_word_tag() {
        let content = "#+filetags: :ai encyclopedia:test:\n";
        let issues = validate_filetags_format(content);
        assert!(
            issues.is_empty(),
            "multi-word tags should be valid, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_validate_filetags_format_double_colon() {
        let content = "#+filetags: :tag1::tag2:\n";
        let issues = validate_filetags_format(content);
        assert!(!issues.is_empty(), "expected double-colon issue");
        assert!(issues[0].1.contains("empty segments"));
    }

    #[test]
    fn test_validate_filetags_format_no_leading_colon() {
        let content = "#+filetags: tag1:tag2:\n";
        let issues = validate_filetags_format(content);
        assert!(!issues.is_empty(), "expected missing leading colon issue");
        assert!(issues[0].1.contains("start and end"));
    }

    #[test]
    fn test_validate_filetags_format_no_trailing_colon() {
        let content = "#+filetags: :tag1:tag2\n";
        let issues = validate_filetags_format(content);
        assert!(!issues.is_empty(), "expected missing trailing colon issue");
        assert!(issues[0].1.contains("start and end"));
    }

    #[test]
    fn test_validate_filetags_multiple_lines() {
        let content =
            "#+filetags: :good:tags:\n#+filetags: :bad::space:\n#+filetags: :also:good:\n";
        let issues = validate_filetags_format(content);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].1.contains("empty segments"));
    }

    #[test]
    fn test_parse_priority() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: priority test

* TODO [#A] High priority :tag1:
Some text
** [#B] Medium sub :tag2:
*** [#C] Low level
*** No priority"#;
        let note = parse_note(content);
        assert_eq!(note.headings.len(), 4);
        assert_eq!(note.headings[0].priority, Some(TaskPriority::A));
        assert_eq!(note.headings[0].todo_state.as_deref(), Some("TODO"));
        assert_eq!(note.headings[1].priority, Some(TaskPriority::B));
        assert!(note.headings[1].todo_state.is_none());
        assert_eq!(note.headings[2].priority, Some(TaskPriority::C));
        assert!(note.headings[3].priority.is_none());
    }

    #[test]
    fn test_parse_scheduled_deadline() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: schedule test

* TODO Task one
SCHEDULED: <2026-05-10 Sun>
Some text
** DONE Subtask
DEADLINE: <2026-06-22 Sun>
More text
* Task two
DEADLINE: <2026-07-01 Wed>
* TODO Combined
SCHEDULED: <2026-05-10 Sun> DEADLINE: <2026-06-22 Sun>"#;
        let note = parse_note(content);
        assert_eq!(note.headings.len(), 4);

        // Task one has SCHEDULED
        assert_eq!(
            note.headings[0].scheduled.as_deref(),
            Some("<2026-05-10 Sun>")
        );
        assert!(note.headings[0].deadline.is_none());

        // Subtask has DEADLINE
        assert_eq!(
            note.headings[1].deadline.as_deref(),
            Some("<2026-06-22 Sun>")
        );
        assert!(note.headings[1].scheduled.is_none());

        // Task two has DEADLINE
        assert_eq!(
            note.headings[2].deadline.as_deref(),
            Some("<2026-07-01 Wed>")
        );

        // Combined has both on the same line
        assert_eq!(
            note.headings[3].scheduled.as_deref(),
            Some("<2026-05-10 Sun>")
        );
        assert_eq!(
            note.headings[3].deadline.as_deref(),
            Some("<2026-06-22 Sun>")
        );
    }

    #[test]
    fn test_parse_scheduled_with_blank_line() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: blank line test

* TODO Task

SCHEDULED: <2026-05-10 Sun>"#;
        let note = parse_note(content);
        assert_eq!(note.headings.len(), 1);
        // Tolerate up to 1 blank line between heading and SCHEDULED/DEADLINE
        assert_eq!(
            note.headings[0].scheduled.as_deref(),
            Some("<2026-05-10 Sun>")
        );
    }

    #[test]
    fn test_has_todo_headings() {
        let content = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: todo test

* TODO Task one
* Done Task two
* Plain heading"#;
        let note = parse_note(content);
        assert!(note.has_todo_headings());

        let content2 = r#":PROPERTIES:
:ID:       a1b2c3d4-e5f6-7890-abcd-ef1234567890
:END:
#+title: no todo test

* Plain heading
** Another one"#;
        let note2 = parse_note(content2);
        assert!(!note2.has_todo_headings());
    }

    #[test]
    fn test_find_daily_file_date() {
        let path = std::path::Path::new("/org/daily/2026-05-03.org");
        assert_eq!(
            find_daily_file_date(path),
            Some(NaiveDate::from_ymd_opt(2026, 5, 3).unwrap())
        );

        let path = std::path::Path::new("/org/notes/regular-note.org");
        assert!(find_daily_file_date(path).is_none());
    }

    proptest! {
        #[test]
        fn test_uuid_format(uuid_str in "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}") {
            let content = format!(":PROPERTIES:\n:ID:       {}\n:END:\n#+title: test", uuid_str);
            let note = parse_note(&content);
            prop_assert_eq!(note.uuids.first().unwrap(), &uuid_str);
        }

        #[test]
        fn test_link_never_panics(content in "\\PC*") {
            let _note = parse_note(&content);
        }
    }
}
