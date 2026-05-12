use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub uuids: Vec<String>,
    pub title: Option<String>,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub roam_aliases: Vec<String>,
    pub roam_refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings: Vec<Heading>,
}

impl ParsedNote {
    pub fn empty() -> Self {
        ParsedNote {
            uuids: vec![],
            title: None,
            filetags: vec![],
            categories: vec![],
            roam_aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        }
    }

    pub fn heading_uuids(&self) -> Vec<String> {
        self.headings
            .iter()
            .filter_map(|h| h.uuid.clone())
            .collect()
    }

    pub fn has_todo_headings(&self) -> bool {
        self.headings.iter().any(|h| {
            h.todo_state
                .as_ref()
                .is_some_and(|s| s.chars().all(|c| c.is_uppercase() || c == '-'))
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Link {
    Internal(String),
    File(String),
    Url(String),
    Attachment(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: usize,
    pub title: String,
    pub todo_state: Option<String>,
    pub tags: Vec<String>,
    pub uuid: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub priority: Option<char>,
    pub outgoing: Vec<Link>,
}

const PROP_ID: &str = "ID";
const PROP_CATEGORY: &str = "CATEGORY";
const PROP_ROAM_ALIASES: &str = "ROAM_ALIASES";
const PROP_ROAM_REFS: &str = "ROAM_REFS";

pub(crate) static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]]+?)(?:\]\[([^\]]*))?\]\]").unwrap());

pub(crate) static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\*+)\s+(?:([A-Z]+)\s+)?(?:\[#([A-C])\]\s+)?(.*?)(?:\s+:(\w+(?::\w+)*):)?\s*$")
        .unwrap()
});

pub(crate) static SCHEDULED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"SCHEDULED:\s*(<[^>]+>)").unwrap());

pub(crate) static DEADLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"DEADLINE:\s*(<[^>]+>)").unwrap());

pub(crate) static DAILY_FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})\.org$").unwrap());

pub(crate) static TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+title:\s*(.*)$").unwrap());

pub(crate) static FILETAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#\+filetags:\s*(.+)$").unwrap());

pub fn parse_note(content: &str) -> ParsedNote {
    let mut uuids = Vec::new();
    let mut title = None;
    let mut filetags = Vec::new();
    let mut categories = Vec::new();
    let mut roam_aliases = Vec::new();
    let mut roam_refs = Vec::new();
    let mut outgoing = Vec::new();
    let mut headings: Vec<Heading> = Vec::new();
    let mut in_properties = false;
    let mut in_src_block = false;
    let mut current_heading_idx: Option<usize> = None;
    let mut just_saw_heading = false;
    let mut heading_stack: Vec<usize> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("#+begin_src") {
            in_src_block = true;
            continue;
        }
        if trimmed == "#+end_src" {
            in_src_block = false;
            continue;
        }
        if in_src_block {
            continue;
        }

        if trimmed == ":PROPERTIES:" {
            in_properties = true;
            continue;
        }
        if trimmed == ":END:" {
            in_properties = false;
            continue;
        }

        if in_properties {
            if let Some((key, value)) = parse_property(trimmed) {
                match key {
                    PROP_ID => {
                        if let Some(idx) = current_heading_idx {
                            headings[idx].uuid = Some(value.to_string());
                        } else {
                            uuids.push(value.to_string());
                        }
                    }
                    PROP_CATEGORY => categories.push(value.to_string()),
                    PROP_ROAM_ALIASES => {
                        roam_aliases = value
                            .split_whitespace()
                            .map(std::string::ToString::to_string)
                            .collect();
                    }
                    PROP_ROAM_REFS => {
                        roam_refs = value
                            .split_whitespace()
                            .map(std::string::ToString::to_string)
                            .collect();
                    }
                    _ => {}
                }
            }
        } else {
            if let Some(cap) = TITLE_RE.captures(line)
                && title.is_none()
            {
                title = Some(cap[1].trim().to_string());
            }

            if let Some(cap) = FILETAGS_RE.captures(line) {
                let tags_str = cap.get(1).map_or("", |m| m.as_str());
                for tag in tags_str.split(':') {
                    let tag = tag.trim();
                    if !tag.is_empty() {
                        filetags.push(tag.to_string());
                    }
                }
            }

            if let Some(cap) = HEADING_RE.captures(line)
                && (cap[1].len() > 1 || cap.get(4).is_some_and(|m| !m.as_str().is_empty()))
            {
                let level = cap[1].len();

                while let Some(&top_idx) = heading_stack.last() {
                    if headings[top_idx].level >= level {
                        heading_stack.pop();
                    } else {
                        break;
                    }
                }

                let todo_state = cap.get(2).map(|m| m.as_str().to_string());
                let priority = cap.get(3).and_then(|m| m.as_str().chars().next());
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
                headings.push(Heading {
                    level,
                    title: heading_title,
                    todo_state,
                    tags,
                    uuid: None,
                    scheduled: None,
                    deadline: None,
                    priority,
                    outgoing: vec![],
                });
                heading_stack.push(headings.len() - 1);
                current_heading_idx = Some(headings.len() - 1);
                just_saw_heading = true;

                for cap in LINK_RE.captures_iter(line) {
                    let link_target = cap[1].to_string();
                    if let Some(link) = parse_link(&link_target) {
                        if let Some(&idx) = heading_stack.last() {
                            headings[idx].outgoing.push(link);
                        } else {
                            outgoing.push(link);
                        }
                    }
                }

                continue;
            }

            if just_saw_heading && !trimmed.is_empty() {
                if let Some(cap) = SCHEDULED_RE.captures(line)
                    && let Some(idx) = current_heading_idx
                {
                    headings[idx].scheduled = cap.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(cap) = DEADLINE_RE.captures(line)
                    && let Some(idx) = current_heading_idx
                {
                    headings[idx].deadline = cap.get(1).map(|m| m.as_str().to_string());
                }
                just_saw_heading = false;
            }

            for cap in LINK_RE.captures_iter(line) {
                let link_target = cap[1].to_string();
                if let Some(link) = parse_link(&link_target) {
                    if let Some(&idx) = heading_stack.last() {
                        headings[idx].outgoing.push(link);
                    } else {
                        outgoing.push(link);
                    }
                }
            }
        }
    }

    ParsedNote {
        uuids,
        title,
        filetags,
        categories,
        roam_aliases,
        roam_refs,
        outgoing,
        headings,
    }
}

pub fn find_daily_file_date(path: &std::path::Path) -> Option<NaiveDate> {
    let filename = path.file_name()?.to_str()?;
    let cap = DAILY_FILE_RE.captures(filename)?;
    NaiveDate::parse_from_str(cap.get(1)?.as_str(), "%Y-%m-%d").ok()
}

fn parse_property(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with(':')
        && let Some(end) = line[1..].find(':')
    {
        let key = &line[1..=end];
        let value = line[end + 2..].trim();
        if !key.is_empty() && !value.is_empty() {
            return Some((key, value));
        }
    }
    None
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
        return Some(Link::Internal(rest.to_string()));
    }
    if let Some(rest) = target.strip_prefix("file:") {
        return Some(Link::File(rest.to_string()));
    }
    if target.starts_with("org:") {
        return Some(Link::File(target.to_string()));
    }
    if let Some(rest) = target.strip_prefix("attachment:") {
        return Some(Link::Attachment(rest.to_string()));
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(Link::Url(target.to_string()));
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
        assert_eq!(note.roam_aliases, vec!["Test", "Alias"]);
        assert_eq!(note.roam_refs, vec!["https://example.com"]);
        assert_eq!(note.outgoing.len(), 4);
        assert!(matches!(note.outgoing[0], Link::Internal(_)));
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
        assert_eq!(note.headings[0].priority, Some('A'));
        assert_eq!(note.headings[0].todo_state.as_deref(), Some("TODO"));
        assert_eq!(note.headings[1].priority, Some('B'));
        assert!(note.headings[1].todo_state.is_none());
        assert_eq!(note.headings[2].priority, Some('C'));
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
        fn test_title_to_slug_roundtrip(title in "[a-zA-Z0-9 _-]{1,50}") {
            let slug = super::super::commands::new::title_to_slug(&title);
            prop_assert!(!slug.is_empty());
            prop_assert!(slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
        }

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
