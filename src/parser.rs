use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub filetags: Vec<String>,
    pub roam_aliases: Vec<String>,
    pub roam_refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings: Vec<Heading>,
}

impl ParsedNote {
    pub fn empty() -> Self {
        ParsedNote {
            uuid: None,
            title: None,
            filetags: vec![],
            roam_aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Link {
    Internal(String),
    File(String),
    Url(String),
    Attachment(String),
}

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: usize,
    pub title: String,
    pub todo_state: Option<String>,
    pub tags: Vec<String>,
}

const PROP_ID: &str = "ID";
const PROP_ROAM_ALIASES: &str = "ROAM_ALIASES";
const PROP_ROAM_REFS: &str = "ROAM_REFS";

pub(crate) static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]]+?)(?:\]\[([^\]]*))?\]\]").unwrap());

pub(crate) static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\*+)\s+(?:(\w+)\s+)?(.*?)(?:\s+:(\w+(?::\w+)*):)?\s*$").unwrap()
});

pub(crate) static TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^#\+title:\s*(.*)$").unwrap());

pub(crate) static FILETAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#\+filetags:\s*(.+)$").unwrap());

pub fn parse_note(content: &str) -> ParsedNote {
    let mut uuid = None;
    let mut title = None;
    let mut filetags = Vec::new();
    let mut roam_aliases = Vec::new();
    let mut roam_refs = Vec::new();
    let mut outgoing = Vec::new();
    let mut headings = Vec::new();
    let mut in_properties = false;

    for line in content.lines() {
        let trimmed = line.trim();

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
                    PROP_ID => uuid = Some(value.to_string()),
                    PROP_ROAM_ALIASES => {
                        roam_aliases =
                            value.split_whitespace().map(|s| s.to_string()).collect()
                    }
                    PROP_ROAM_REFS => {
                        roam_refs =
                            value.split_whitespace().map(|s| s.to_string()).collect()
                    }
                    _ => {}
                }
            }
        } else {
            if let Some(cap) = TITLE_RE.captures(line) {
                if title.is_none() {
                    title = Some(cap[1].trim().to_string());
                }
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

            if let Some(cap) = HEADING_RE.captures(line) {
                if cap[1].len() > 1 || cap.get(3).map_or(false, |m| !m.as_str().is_empty()) {
                    let level = cap[1].len();
                    let todo_state = cap.get(2).map(|m| m.as_str().to_string());
                    let heading_title = cap.get(3).map_or("", |m| m.as_str()).to_string();
                    let tags = cap
                        .get(4)
                        .map(|m| {
                            m.as_str()
                                .split(':')
                                .filter(|t| !t.is_empty())
                                .map(|t| t.to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    headings.push(Heading {
                        level,
                        title: heading_title,
                        todo_state,
                        tags,
                    });
                }
            }

            for cap in LINK_RE.captures_iter(line) {
                let link_target = cap[1].to_string();
                if let Some(link) = parse_link(&link_target) {
                    outgoing.push(link);
                }
            }
        }
    }

    ParsedNote {
        uuid,
        title,
        filetags,
        roam_aliases,
        roam_refs,
        outgoing,
        headings,
    }
}

fn parse_property(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with(':') {
        if let Some(end) = line[1..].find(':') {
            let key = &line[1..=end];
            let value = line[end + 2..].trim();
            if !key.is_empty() && !value.is_empty() {
                return Some((key, value));
            }
        }
    }
    None
}

fn parse_link(target: &str) -> Option<Link> {
    if let Some(rest) = target.strip_prefix("id:") {
        return Some(Link::Internal(rest.to_string()));
    }
    if let Some(rest) = target.strip_prefix("file:") {
        return Some(Link::File(rest.to_string()));
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
        assert_eq!(note.uuid.unwrap(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
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
            prop_assert_eq!(note.uuid.unwrap(), uuid_str);
        }

        #[test]
        fn test_link_never_panics(content in "\\PC*") {
            let _note = parse_note(&content);
        }
    }
}
