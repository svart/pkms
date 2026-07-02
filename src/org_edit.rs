use anyhow::Result;
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
}
