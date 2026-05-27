use crate::parser::HEADING_RE;
use anyhow::{Context, Result};
use std::path::Path;

pub fn append_org_entry(path: &Path, entry: &str) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read inbox note: {}", path.display()))?;
    let line_number = append_org_entry_to_content(&mut content, entry);
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write inbox note: {}", path.display()))?;
    Ok(line_number)
}

pub fn append_daily_inbox_entry(path: &Path, entry: &str) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read daily note: {}", path.display()))?;
    let line_number = append_daily_inbox_entry_to_content(&mut content, entry);
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write daily note: {}", path.display()))?;
    Ok(line_number)
}

pub fn inbox_section_range(path: &Path) -> Result<Option<(usize, usize)>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read daily note: {}", path.display()))?;
    Ok(inbox_section_range_from_content(&content))
}

fn append_org_entry_to_content(content: &mut String, entry: &str) -> usize {
    if !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.ends_with("\n\n") {
        content.push('\n');
    }
    let line_number = content.lines().count() + 1;
    content.push_str(entry);
    line_number
}

fn append_daily_inbox_entry_to_content(content: &mut String, entry: &str) -> usize {
    normalize_trailing_newline(content);

    if let Some((_start, end)) = inbox_section_range_from_content(content) {
        let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
        let insert_idx = end.saturating_sub(1);
        lines.insert(insert_idx, entry.to_string());
        *content = lines.concat();
        return insert_idx + 1;
    }

    if !content.ends_with("\n\n") {
        content.push('\n');
    }
    let inbox_heading_line = content.lines().count() + 1;
    content.push_str("* Inbox\n");
    content.push_str(entry);
    inbox_heading_line + 1
}

fn normalize_trailing_newline(content: &mut String) {
    if !content.ends_with('\n') {
        content.push('\n');
    }
}

fn inbox_section_range_from_content(content: &str) -> Option<(usize, usize)> {
    let mut start = None;
    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;
        let Some(captures) = HEADING_RE.captures(line) else {
            continue;
        };
        let level = captures.get(1).map_or("", |m| m.as_str()).len();
        if level != 1 {
            continue;
        }
        if start.is_some() {
            return Some((start?, line_number));
        }
        let title = captures.get(4).map_or("", |m| m.as_str()).trim();
        if title.eq_ignore_ascii_case("Inbox") {
            start = Some(line_number);
        }
    }
    start.map(|start| (start, content.lines().count() + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_regular_note_entry_after_blank_line() {
        let mut content = "#+title: Inbox\n".to_string();
        let line = append_org_entry_to_content(&mut content, "* TODO Call\n");
        assert_eq!(line, 3);
        assert_eq!(content, "#+title: Inbox\n\n* TODO Call\n");
    }

    #[test]
    fn inserts_daily_entry_inside_existing_inbox_section() {
        let mut content = "#+title: Day\n\n* Inbox\n** TODO Existing\n* Later\n".to_string();
        let line = append_daily_inbox_entry_to_content(&mut content, "** TODO New\n");
        assert_eq!(line, 5);
        assert_eq!(
            content,
            "#+title: Day\n\n* Inbox\n** TODO Existing\n** TODO New\n* Later\n"
        );
    }

    #[test]
    fn creates_daily_inbox_section_when_missing() {
        let mut content = "#+title: Day\n".to_string();
        let line = append_daily_inbox_entry_to_content(&mut content, "** TODO New\n");
        assert_eq!(line, 4);
        assert_eq!(content, "#+title: Day\n\n* Inbox\n** TODO New\n");
    }
}
