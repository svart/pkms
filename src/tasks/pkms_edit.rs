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

pub fn heading_level_at(path: &Path, line_number: usize) -> Result<usize> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read task note: {}", path.display()))?;
    heading_level_in_content(&content, line_number)
}

pub fn append_child_org_entry(
    path: &Path,
    parent_line_number: usize,
    entry: &str,
) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read task note: {}", path.display()))?;
    let line_number = append_child_org_entry_to_content(&mut content, parent_line_number, entry)?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write task note: {}", path.display()))?;
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

fn append_child_org_entry_to_content(
    content: &mut String,
    parent_line_number: usize,
    entry: &str,
) -> Result<usize> {
    normalize_trailing_newline(content);
    let parent_level = heading_level_in_content(content, parent_line_number)?;
    let parent_idx = parent_line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {parent_line_number}"))?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    let mut insert_idx = lines.len();

    for (idx, line) in lines.iter().enumerate().skip(parent_idx + 1) {
        let body = line.trim_end();
        let Some(captures) = HEADING_RE.captures(body) else {
            continue;
        };
        let level = captures.get(1).map_or("", |m| m.as_str()).len();
        if level <= parent_level {
            insert_idx = idx;
            break;
        }
    }

    lines.insert(insert_idx, entry.to_string());
    *content = lines.concat();
    Ok(insert_idx + 1)
}

fn heading_level_in_content(content: &str, line_number: usize) -> Result<usize> {
    let line_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let line = content
        .lines()
        .nth(line_idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists"))?;
    let captures = HEADING_RE
        .captures(line)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} is no longer an org heading"))?;
    Ok(captures.get(1).map_or("", |m| m.as_str()).len())
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

    #[test]
    fn appends_child_entry_at_end_of_parent_subtree() {
        let mut content = "#+title: Tasks\n\n* Project\n** TODO Parent\nBody\n*** TODO Child\nChild body\n* TODO Sibling\n".to_string();

        let line =
            append_child_org_entry_to_content(&mut content, 4, "*** TODO New child\n").unwrap();

        assert_eq!(line, 8);
        assert_eq!(
            content,
            "#+title: Tasks\n\n* Project\n** TODO Parent\nBody\n*** TODO Child\nChild body\n*** TODO New child\n* TODO Sibling\n"
        );
    }

    #[test]
    fn appends_child_entry_at_end_of_file_parent_subtree() {
        let mut content = "#+title: Tasks\n\n** TODO Parent\nBody".to_string();

        let line =
            append_child_org_entry_to_content(&mut content, 3, "*** TODO New child\n").unwrap();

        assert_eq!(line, 5);
        assert_eq!(
            content,
            "#+title: Tasks\n\n** TODO Parent\nBody\n*** TODO New child\n"
        );
    }
}
