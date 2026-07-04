use anyhow::{Context, Result};
use pkms_org::{Heading, Link, ParsedNote};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::{ChunkRecord, SUPPORTED_SCHEMA_VERSION};

pub const CONTENT_HASH_PREFIX: &str = "sha256:";

#[derive(Debug, Clone, Copy)]
pub struct ChunkNoteInput<'a> {
    pub note_id: &'a str,
    pub path: &'a str,
    pub title: &'a str,
    pub aliases: &'a [String],
    pub tags: &'a [String],
    pub updated_at: i64,
    pub parsed: &'a ParsedNote,
    pub raw_content: &'a str,
}

#[derive(Debug, Clone)]
struct Section {
    heading_index: Option<usize>,
    heading_path: Vec<String>,
    heading_level: u32,
    lines: Vec<NumberedLine>,
}

#[derive(Debug, Clone)]
struct NumberedLine {
    number: usize,
    text: String,
}

#[derive(Debug, Default)]
struct LineFilterState {
    in_property_drawer: bool,
    in_src_block: bool,
}

#[derive(Serialize)]
struct ChunkHashInput<'a> {
    note_id: &'a str,
    path: &'a str,
    title: &'a str,
    aliases: &'a [String],
    tags: &'a [String],
    heading_path: &'a [String],
    heading_level: u32,
    heading_id: Option<&'a str>,
    todo_state: Option<&'a str>,
    body: &'a str,
    start_line: u32,
    end_line: u32,
    outgoing_ids: &'a [String],
}

pub fn chunk_note(input: ChunkNoteInput<'_>) -> Result<Vec<ChunkRecord>> {
    let mut heading_order = input.parsed.headings.iter().enumerate().collect::<Vec<_>>();
    heading_order.sort_by_key(|(_, heading)| heading.line_number);

    let mut chunks = Vec::new();
    let mut heading_cursor = 0;
    let mut heading_stack: Vec<usize> = Vec::new();
    let mut section = Section::body();
    let mut line_filter = LineFilterState::default();

    for (line_index, line) in input.raw_content.lines().enumerate() {
        let line_number = line_index + 1;
        if heading_cursor < heading_order.len()
            && heading_order[heading_cursor].1.line_number == line_number
        {
            push_section_chunk(input, &mut chunks, &section)?;
            let (heading_index, heading) = heading_order[heading_cursor];
            while heading_stack
                .last()
                .is_some_and(|idx| input.parsed.headings[*idx].level >= heading.level)
            {
                heading_stack.pop();
            }
            heading_stack.push(heading_index);
            section = Section::heading(&heading_stack, input.parsed)?;
            line_filter.in_property_drawer = false;
            heading_cursor += 1;
        }

        if should_keep_line(line, &mut line_filter) {
            section.lines.push(NumberedLine {
                number: line_number,
                text: line.to_string(),
            });
        }
    }

    push_section_chunk(input, &mut chunks, &section)?;
    Ok(chunks)
}

pub fn content_hash(text: &str) -> String {
    sha256_prefixed(text.as_bytes())
}

pub fn clean_list(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn push_section_chunk(
    input: ChunkNoteInput<'_>,
    chunks: &mut Vec<ChunkRecord>,
    section: &Section,
) -> Result<()> {
    let Some((start_line, end_line, body)) = section.trimmed_body() else {
        return Ok(());
    };
    let heading = section.heading_index.map(|idx| &input.parsed.headings[idx]);
    let tags = section_tags(input.tags, heading);
    let outgoing_ids = section_outgoing_ids(input.parsed, heading);
    let start_line = u32::try_from(start_line).context("chunk start line exceeds u32")?;
    let end_line = u32::try_from(end_line).context("chunk end line exceeds u32")?;
    let heading_id = heading.and_then(|heading| heading.uuid.as_ref().map(|id| id.as_str()));
    let todo_state = heading.and_then(|heading| heading.todo_state.as_deref());

    let hash_input = ChunkHashInput {
        note_id: input.note_id,
        path: input.path,
        title: input.title,
        aliases: input.aliases,
        tags: &tags,
        heading_path: &section.heading_path,
        heading_level: section.heading_level,
        heading_id,
        todo_state,
        body: &body,
        start_line,
        end_line,
        outgoing_ids: &outgoing_ids,
    };
    let hash_bytes =
        serde_json::to_vec(&hash_input).context("failed to encode chunk hash input")?;
    let content_hash = sha256_prefixed(&hash_bytes);
    let suffix = content_hash
        .strip_prefix(CONTENT_HASH_PREFIX)
        .unwrap_or(&content_hash)
        .chars()
        .take(12)
        .collect::<String>();
    let slug = heading_id
        .map(|id| format!("heading-{id}"))
        .unwrap_or_else(|| slugify_heading_path(&section.heading_path));

    chunks.push(ChunkRecord {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        chunk_id: format!("{}:{}:{}", input.note_id, slug, suffix),
        note_id: input.note_id.to_string(),
        path: input.path.to_string(),
        title: input.title.to_string(),
        aliases: input.aliases.to_vec(),
        tags,
        heading_path: section.heading_path.clone(),
        heading_level: section.heading_level,
        body,
        start_line,
        end_line,
        outgoing_ids,
        updated_at: input.updated_at,
        content_hash,
    });

    Ok(())
}

impl Section {
    fn body() -> Self {
        Self {
            heading_index: None,
            heading_path: Vec::new(),
            heading_level: 0,
            lines: Vec::new(),
        }
    }

    fn heading(heading_stack: &[usize], parsed: &ParsedNote) -> Result<Self> {
        let Some(&heading_index) = heading_stack.last() else {
            return Ok(Self::body());
        };
        let heading = &parsed.headings[heading_index];
        Ok(Self {
            heading_index: Some(heading_index),
            heading_path: heading_stack
                .iter()
                .map(|idx| parsed.headings[*idx].title.clone())
                .collect(),
            heading_level: u32::try_from(heading.level).context("heading level exceeds u32")?,
            lines: Vec::new(),
        })
    }

    fn trimmed_body(&self) -> Option<(usize, usize, String)> {
        let first = self
            .lines
            .iter()
            .position(|line| !line.text.trim().is_empty())?;
        let last = self
            .lines
            .iter()
            .rposition(|line| !line.text.trim().is_empty())?;
        let lines = &self.lines[first..=last];
        let body = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let start_line = lines.first()?.number;
        let end_line = lines.last()?.number;
        Some((start_line, end_line, body))
    }
}

fn should_keep_line(line: &str, state: &mut LineFilterState) -> bool {
    let trimmed = line.trim();

    if state.in_src_block {
        if trimmed.eq_ignore_ascii_case("#+end_src") {
            state.in_src_block = false;
        }
        return true;
    }

    if starts_with_ignore_ascii_case(trimmed, "#+begin_src") {
        state.in_src_block = true;
        return true;
    }

    if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
        state.in_property_drawer = true;
        return false;
    }

    if state.in_property_drawer {
        if trimmed.eq_ignore_ascii_case(":END:") {
            state.in_property_drawer = false;
        }
        return false;
    }

    !starts_with_ignore_ascii_case(trimmed, "#+")
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn section_tags(note_tags: &[String], heading: Option<&Heading>) -> Vec<String> {
    let heading_tags = heading
        .into_iter()
        .flat_map(|heading| heading.tags.iter().cloned());
    clean_list(note_tags.iter().cloned().chain(heading_tags))
}

fn section_outgoing_ids(parsed: &ParsedNote, heading: Option<&Heading>) -> Vec<String> {
    let links = heading.map_or(parsed.outgoing.as_slice(), |heading| {
        heading.outgoing.as_slice()
    });
    clean_list(links.iter().filter_map(|link| match link {
        Link::Internal(id) => Some(id.as_str().to_string()),
        Link::File(_) | Link::Url(_) | Link::Attachment(_) => None,
    }))
}

fn slugify_heading_path(heading_path: &[String]) -> String {
    let value = if heading_path.is_empty() {
        "body".to_string()
    } else {
        heading_path.join(" ")
    };
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut value = String::with_capacity(CONTENT_HASH_PREFIX.len() + digest.len() * 2);
    value.push_str(CONTENT_HASH_PREFIX);
    for byte in digest {
        value.push(nibble_to_hex(byte >> 4));
        value.push(nibble_to_hex(byte & 0x0f));
    }
    value
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + value - 10) as char,
        _ => unreachable!("nibble is always <= 15"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkms_org::parser::parse_note;

    const NOTE_ID: &str = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";

    #[test]
    fn chunking_excludes_drawers_and_metadata_but_preserves_source_blocks() {
        let raw = "\
#+title: Snippets
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:SECRET: hidden
:END:
* Snippet
:PROPERTIES:
:ID: dddddddd-dddd-4ddd-dddd-dddddddddddd
:SECRET: hidden heading
:END:
#+caption: hidden caption
#+begin_src org
:PROPERTIES:
:ID: not-parsed-from-source
#+title: kept in source
#+end_src
Plain text.
";
        let parsed = parse_note(raw);
        let tags = vec!["rag".to_string()];

        let chunks = chunk_note(input(&parsed, raw, &tags)).expect("chunks build");

        assert_eq!(chunks.len(), 1);
        let body = &chunks[0].body;
        assert!(body.contains("* Snippet"));
        assert!(body.contains("#+begin_src org"));
        assert!(body.contains(":ID: not-parsed-from-source"));
        assert!(body.contains("#+title: kept in source"));
        assert!(body.contains("Plain text."));
        assert!(!body.contains(":SECRET: hidden"));
        assert!(!body.contains(":SECRET: hidden heading"));
        assert!(!body.contains("#+caption: hidden caption"));
    }

    #[test]
    fn chunking_uses_nested_heading_paths_tags_heading_ids_and_outgoing_ids() {
        let raw = "\
#+title: Nested
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Parent :area:
Parent text.
** TODO Child :task:
:PROPERTIES:
:ID: dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
See [[id:eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee][target]].
";
        let parsed = parse_note(raw);
        let tags = vec!["rag".to_string()];

        let chunks = chunk_note(input(&parsed, raw, &tags)).expect("chunks build");

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading_path, vec!["Parent"]);
        assert_eq!(chunks[0].tags, vec!["area", "rag"]);
        assert_eq!(chunks[0].start_line, 5);
        assert_eq!(chunks[0].end_line, 6);

        let child = &chunks[1];
        assert_eq!(child.heading_path, vec!["Parent", "Child"]);
        assert_eq!(child.heading_level, 2);
        assert_eq!(child.tags, vec!["rag", "task"]);
        assert_eq!(
            child.outgoing_ids,
            vec!["eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"]
        );
        assert!(
            child
                .chunk_id
                .contains("heading-dddddddd-dddd-4ddd-dddd-dddddddddddd")
        );
        assert!(child.body.contains("** TODO Child :task:"));
        assert!(!child.body.contains("dddddddd-dddd-4ddd-dddd-dddddddddddd"));
    }

    #[test]
    fn chunk_ids_are_stable_and_change_when_body_changes() {
        let raw = "\
#+title: Stable
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Section
Original body.
";
        let parsed = parse_note(raw);
        let tags = vec!["rag".to_string()];
        let chunks = chunk_note(input(&parsed, raw, &tags)).expect("chunks build");
        let repeated = chunk_note(input(&parsed, raw, &tags)).expect("chunks build again");

        assert_eq!(chunks[0].chunk_id, repeated[0].chunk_id);
        assert_eq!(chunks[0].content_hash, repeated[0].content_hash);

        let changed = raw.replace("Original body.", "Changed body.");
        let changed_parsed = parse_note(&changed);
        let changed_chunks =
            chunk_note(input(&changed_parsed, &changed, &tags)).expect("changed chunks");

        assert_ne!(chunks[0].content_hash, changed_chunks[0].content_hash);
        assert_ne!(chunks[0].chunk_id, changed_chunks[0].chunk_id);
    }

    fn input<'a>(
        parsed: &'a ParsedNote,
        raw_content: &'a str,
        tags: &'a [String],
    ) -> ChunkNoteInput<'a> {
        ChunkNoteInput {
            note_id: NOTE_ID,
            path: "nested.org",
            title: "Nested",
            aliases: &[],
            tags,
            updated_at: 42,
            parsed,
            raw_content,
        }
    }
}
