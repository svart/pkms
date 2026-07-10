use anyhow::{Context, Result, ensure};
use std::path::Path;

use crate::parser::{HEADING_RE, Heading};

pub fn read_lines(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if content.is_empty() || !content.ends_with('\n') {
        let consumed: usize = lines.iter().map(String::len).sum();
        if consumed < content.len() {
            lines.push(content[consumed..].to_string());
        }
    }
    Ok(lines)
}

pub fn write_lines(path: impl AsRef<Path>, lines: &[String]) -> Result<()> {
    std::fs::write(path, lines.concat())?;
    Ok(())
}

pub fn update_filetags(path: impl AsRef<Path>, tags: &[String]) -> Result<bool> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read note: {}", path.display()))?;
    let updated = update_filetags_in_content(&content, tags)?;
    if updated == content {
        return Ok(false);
    }
    std::fs::write(path, updated)
        .with_context(|| format!("Failed to write note: {}", path.display()))?;
    Ok(true)
}

fn update_filetags_in_content(content: &str, tags: &[String]) -> Result<String> {
    let mut tags = tags
        .iter()
        .map(|tag| tag.trim().to_string())
        .collect::<Vec<_>>();
    for tag in &tags {
        ensure!(
            !tag.is_empty() && tag.chars().all(|ch| ch != ':' && !ch.is_whitespace()),
            "Invalid org tag '{tag}': tags cannot be empty or contain colons or whitespace"
        );
    }
    tags.sort();
    tags.dedup();
    ensure!(!tags.is_empty(), "At least one org tag is required");

    let mut lines = content
        .split_inclusive('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    if content.is_empty() {
        lines.clear();
    }
    let newline = lines
        .iter()
        .find_map(|line| {
            let (_, newline) = split_line_ending(line);
            (!newline.is_empty()).then_some(newline)
        })
        .unwrap_or("\n");
    let directive = format!("#+filetags: :{}:", tags.join(":"));

    if let Some(index) = org_keyword_position(&lines, "#+filetags:") {
        let (_, ending) = split_line_ending(&lines[index]);
        lines[index] = format!("{directive}{ending}");
        return Ok(lines.concat());
    }

    let insert_index = org_keyword_position(&lines, "#+title:")
        .map(|index| index + 1)
        .unwrap_or_else(|| initial_property_drawer_end(&lines).unwrap_or(0));
    if insert_index > 0 {
        let previous = &mut lines[insert_index - 1];
        if split_line_ending(previous).1.is_empty() {
            previous.push_str(newline);
        }
    }
    let ending = if insert_index < lines.len() || content.ends_with('\n') {
        newline
    } else {
        ""
    };
    lines.insert(insert_index, format!("{directive}{ending}"));
    Ok(lines.concat())
}

fn is_org_keyword(line: &str, keyword: &str) -> bool {
    let (body, _) = split_line_ending(line);
    body.get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
}

fn org_keyword_position(lines: &[String], keyword: &str) -> Option<usize> {
    let mut in_source_block = false;
    for (index, line) in lines.iter().enumerate() {
        if in_source_block {
            if is_org_keyword(line, "#+end_src") {
                in_source_block = false;
            }
            continue;
        }
        if is_org_keyword(line, "#+begin_src") {
            in_source_block = true;
            continue;
        }
        if is_org_keyword(line, keyword) {
            return Some(index);
        }
    }
    None
}

fn initial_property_drawer_end(lines: &[String]) -> Option<usize> {
    let start = lines
        .iter()
        .position(|line| !split_line_ending(line).0.trim().is_empty())?;
    if !split_line_ending(&lines[start])
        .0
        .trim()
        .eq_ignore_ascii_case(":PROPERTIES:")
    {
        return None;
    }
    lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            split_line_ending(line)
                .0
                .trim()
                .eq_ignore_ascii_case(":END:")
        })
        .map(|(index, _)| index + 1)
}

pub fn split_line_ending(line: &str) -> (&str, &'static str) {
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    (line.strip_suffix(newline).unwrap_or(line), newline)
}

pub fn heading_level(line: &str) -> Option<usize> {
    let (body, _) = split_line_ending(line);
    HEADING_RE
        .captures(body)
        .map(|captures| captures.get(1).map_or("", |matched| matched.as_str()).len())
}

pub fn is_heading_line(line: &str) -> bool {
    heading_level(line).is_some()
}

pub fn heading_level_at_index(
    lines: &[String],
    line_idx: usize,
    line_number: usize,
    label: &str,
) -> Result<usize> {
    let line = lines
        .get(line_idx)
        .ok_or_else(|| anyhow::anyhow!("{label} line {line_number} no longer exists"))?;
    heading_level(line)
        .ok_or_else(|| anyhow::anyhow!("{label} line {line_number} is no longer an org heading"))
}

pub fn heading_subtree_end_index(lines: &[String], heading_idx: usize, root_level: usize) -> usize {
    for (idx, line) in lines.iter().enumerate().skip(heading_idx + 1) {
        if heading_level(line).is_some_and(|level| level <= root_level) {
            return idx;
        }
    }
    lines.len()
}

pub fn parsed_heading_subtree_end_index(
    headings: &[Heading],
    target: &Heading,
    total_lines: usize,
) -> usize {
    headings
        .iter()
        .find(|heading| heading.line_number > target.line_number && heading.level <= target.level)
        .map_or(total_lines, |heading| heading.line_number.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_heading_levels_with_line_endings() {
        assert_eq!(heading_level("** TODO Task\r\n"), Some(2));
        assert_eq!(heading_level("Body\n"), None);
    }

    #[test]
    fn raw_subtree_end_stops_at_same_or_lower_heading() {
        let lines = "* Parent\nBody\n** Child\nChild body\n*** Grandchild\n** Sibling\n"
            .split_inclusive('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(heading_subtree_end_index(&lines, 2, 2), 5);
    }

    #[test]
    fn filetags_replace_existing_directive_canonically() {
        let content = ":PROPERTIES:\r\n:ID: note-id\r\n:END:\r\n#+title: Note\r\n#+FILETAGS: :old:\r\n\r\nBody\r\n";

        let updated = update_filetags_in_content(
            content,
            &["zeta".to_string(), "alpha".to_string(), "zeta".to_string()],
        )
        .expect("filetags update");

        assert_eq!(
            updated,
            ":PROPERTIES:\r\n:ID: note-id\r\n:END:\r\n#+title: Note\r\n#+filetags: :alpha:zeta:\r\n\r\nBody\r\n"
        );
    }

    #[test]
    fn filetags_insert_after_title_when_missing() {
        let content = ":PROPERTIES:\n:ID: note-id\n:END:\n#+title: Note\n\nBody\n";

        let updated =
            update_filetags_in_content(content, &["rag".to_string()]).expect("filetags update");

        assert_eq!(
            updated,
            ":PROPERTIES:\n:ID: note-id\n:END:\n#+title: Note\n#+filetags: :rag:\n\nBody\n"
        );
    }

    #[test]
    fn filetags_insert_after_initial_property_drawer_without_title() {
        let content = ":PROPERTIES:\n:ID: note-id\n:END:\n\n* Heading\n";

        let updated =
            update_filetags_in_content(content, &["rag".to_string()]).expect("filetags update");

        assert_eq!(
            updated,
            ":PROPERTIES:\n:ID: note-id\n:END:\n#+filetags: :rag:\n\n* Heading\n"
        );
    }

    #[test]
    fn filetags_reject_noncanonical_tag_values() {
        let error = update_filetags_in_content("#+title: Note\n", &["bad tag".to_string()])
            .expect_err("invalid tag rejected");

        assert!(error.to_string().contains("Invalid org tag"));
    }

    #[test]
    fn filetags_ignore_keyword_examples_inside_source_blocks() {
        let content = "#+title: Note\n#+begin_src org\n#+filetags: :example:\n#+end_src\nBody\n";

        let updated =
            update_filetags_in_content(content, &["rag".to_string()]).expect("filetags update");

        assert_eq!(
            updated,
            "#+title: Note\n#+filetags: :rag:\n#+begin_src org\n#+filetags: :example:\n#+end_src\nBody\n"
        );
    }
}
