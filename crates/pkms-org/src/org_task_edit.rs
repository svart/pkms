use crate::org_date::format_org_date;
use crate::org_edit::{heading_level, heading_level_at_index, heading_subtree_end_index};
use crate::parser::{HEADING_RE, OrgPriority, OrgTodoState};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgTaskInsertSpec {
    pub level: usize,
    pub state: OrgTodoState,
    pub title: String,
    pub priority: Option<OrgPriority>,
    pub tags: Vec<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub project: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgDailyNoteSpec {
    pub path: PathBuf,
    pub date: NaiveDate,
    pub uuid: String,
}

pub fn append_org_task(path: &Path, spec: &OrgTaskInsertSpec) -> Result<usize> {
    let entry = format_org_task_entry(spec)?;
    append_org_entry(path, &entry)
}

pub fn append_daily_inbox_task(path: &Path, spec: &OrgTaskInsertSpec) -> Result<usize> {
    let entry = format_org_task_entry(spec)?;
    append_daily_inbox_entry(path, &entry)
}

pub fn append_child_org_task(
    path: &Path,
    parent_line_number: usize,
    spec: &OrgTaskInsertSpec,
) -> Result<usize> {
    let entry = format_org_task_entry(spec)?;
    append_child_org_entry(path, parent_line_number, &entry)
}

pub fn ensure_daily_note_exists(spec: &OrgDailyNoteSpec) -> Result<()> {
    if let Some(parent) = spec.path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create daily note directory: {}",
                parent.display()
            )
        })?;
    }
    if !spec.path.exists() {
        let title = spec.date.format("%Y-%m-%d").to_string();
        std::fs::write(
            &spec.path,
            format!(
                ":PROPERTIES:\n:ID:       {}\n:END:\n#+title: {title}\n\n",
                spec.uuid
            ),
        )
        .with_context(|| format!("Failed to create daily note: {}", spec.path.display()))?;
    }
    Ok(())
}

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

pub fn move_org_subtree(
    source_path: &Path,
    source_line_number: usize,
    target_path: &Path,
    target_line_number: usize,
) -> Result<usize> {
    if same_existing_path(source_path, target_path) {
        return move_org_subtree_in_file(source_path, source_line_number, target_line_number);
    }
    move_org_subtree_between_files(
        source_path,
        source_line_number,
        target_path,
        target_line_number,
    )
}

pub fn remove_org_subtree_dependency(
    path: &Path,
    source_line_number: usize,
    parent_line_number: usize,
) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read task note: {}", path.display()))?;
    normalize_trailing_newline(&mut content);
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    let new_line =
        remove_org_subtree_dependency_in_lines(&mut lines, source_line_number, parent_line_number)?;
    std::fs::write(path, lines.concat())
        .with_context(|| format!("Failed to write task note: {}", path.display()))?;
    Ok(new_line)
}

pub fn inbox_section_range(path: &Path) -> Result<Option<(usize, usize)>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read daily note: {}", path.display()))?;
    Ok(inbox_section_range_from_content(&content))
}

fn format_org_task_entry(spec: &OrgTaskInsertSpec) -> Result<String> {
    if spec.level == 0 {
        anyhow::bail!("Org task heading level must be at least 1.");
    }

    let mut entry = format!("{}\n", format_org_task_heading(spec));
    if let Some(planning) = format_org_task_planning(spec)? {
        entry.push_str(&planning);
        entry.push('\n');
    }
    if let Some(project) = spec.project.as_deref() {
        entry.push_str(":PROPERTIES:\n:PROJECT: ");
        entry.push_str(project);
        entry.push_str("\n:END:\n");
    }
    if let Some(description) = format_org_task_description(spec.description.as_deref()) {
        entry.push('\n');
        entry.push_str(description);
        entry.push('\n');
    }

    Ok(entry)
}

fn format_org_task_heading(spec: &OrgTaskInsertSpec) -> String {
    let priority = spec
        .priority
        .map(|priority| format!(" [#{priority}]"))
        .unwrap_or_default();
    let tags = if spec.tags.is_empty() {
        String::new()
    } else {
        format!(" :{}:", spec.tags.join(":"))
    };
    format!(
        "{} {}{} {}{}",
        "*".repeat(spec.level),
        spec.state,
        priority,
        spec.title,
        tags
    )
}

fn format_org_task_planning(spec: &OrgTaskInsertSpec) -> Result<Option<String>> {
    let mut planning = Vec::new();
    if let Some(scheduled) = &spec.scheduled {
        planning.push(format!("SCHEDULED: {}", format_org_date(scheduled)?));
    }
    if let Some(deadline) = &spec.deadline {
        planning.push(format!("DEADLINE: {}", format_org_date(deadline)?));
    }
    Ok((!planning.is_empty()).then(|| planning.join(" ")))
}

fn format_org_task_description(description: Option<&str>) -> Option<&str> {
    description
        .map(str::trim)
        .filter(|description| !description.is_empty())
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
        if heading_level(line).is_some_and(|level| level <= parent_level) {
            insert_idx = idx;
            break;
        }
    }

    lines.insert(insert_idx, entry.to_string());
    *content = lines.concat();
    Ok(insert_idx + 1)
}

fn move_org_subtree_in_file(
    path: &Path,
    source_line_number: usize,
    target_line_number: usize,
) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read task note: {}", path.display()))?;
    normalize_trailing_newline(&mut content);
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    let new_line = move_org_subtree_in_lines(&mut lines, source_line_number, target_line_number)?;
    std::fs::write(path, lines.concat())
        .with_context(|| format!("Failed to write task note: {}", path.display()))?;
    Ok(new_line)
}

fn move_org_subtree_between_files(
    source_path: &Path,
    source_line_number: usize,
    target_path: &Path,
    target_line_number: usize,
) -> Result<usize> {
    let mut source_content = std::fs::read_to_string(source_path)
        .with_context(|| format!("Failed to read task note: {}", source_path.display()))?;
    let mut target_content = std::fs::read_to_string(target_path)
        .with_context(|| format!("Failed to read task note: {}", target_path.display()))?;
    let original_target_content = target_content.clone();
    normalize_trailing_newline(&mut source_content);
    normalize_trailing_newline(&mut target_content);

    let mut source_lines: Vec<String> = source_content
        .split_inclusive('\n')
        .map(str::to_string)
        .collect();
    let mut target_lines: Vec<String> = target_content
        .split_inclusive('\n')
        .map(str::to_string)
        .collect();

    let source_idx = line_index(source_line_number, "source task")?;
    let source_level = heading_level_in_lines(&source_lines, source_idx, source_line_number)?;
    let source_end = heading_subtree_end_index(&source_lines, source_idx, source_level);
    let mut subtree = source_lines[source_idx..source_end].to_vec();
    source_lines.drain(source_idx..source_end);

    let target_idx = line_index(target_line_number, "target task")?;
    let target_level = heading_level_in_lines(&target_lines, target_idx, target_line_number)?;
    relevel_subtree_lines(&mut subtree, target_level + 1, source_level)?;
    let insert_idx = heading_subtree_end_index(&target_lines, target_idx, target_level);
    target_lines.splice(insert_idx..insert_idx, subtree);

    std::fs::write(target_path, target_lines.concat())
        .with_context(|| format!("Failed to write task note: {}", target_path.display()))?;
    if let Err(error) = std::fs::write(source_path, source_lines.concat()) {
        let rollback = std::fs::write(target_path, original_target_content);
        if let Err(rollback_error) = rollback {
            return Err(error).with_context(|| {
                format!(
                    "Failed to write task note: {}; also failed to restore {}: {}",
                    source_path.display(),
                    target_path.display(),
                    rollback_error
                )
            });
        }
        return Err(error)
            .with_context(|| format!("Failed to write task note: {}", source_path.display()));
    }
    Ok(insert_idx + 1)
}

fn move_org_subtree_in_lines(
    lines: &mut Vec<String>,
    source_line_number: usize,
    target_line_number: usize,
) -> Result<usize> {
    let source_idx = line_index(source_line_number, "source task")?;
    let target_idx = line_index(target_line_number, "target task")?;
    if source_idx == target_idx {
        anyhow::bail!("Cannot move a task under itself.");
    }

    let source_level = heading_level_in_lines(lines, source_idx, source_line_number)?;
    let target_level = heading_level_in_lines(lines, target_idx, target_line_number)?;
    let source_end = heading_subtree_end_index(lines, source_idx, source_level);
    if (source_idx..source_end).contains(&target_idx) {
        anyhow::bail!("Cannot move a task under one of its descendants.");
    }

    let mut subtree = lines[source_idx..source_end].to_vec();
    relevel_subtree_lines(&mut subtree, target_level + 1, source_level)?;
    let removed_len = source_end - source_idx;
    lines.drain(source_idx..source_end);

    let adjusted_target_idx = if source_idx < target_idx {
        target_idx - removed_len
    } else {
        target_idx
    };
    let insert_idx = heading_subtree_end_index(lines, adjusted_target_idx, target_level);
    lines.splice(insert_idx..insert_idx, subtree);
    Ok(insert_idx + 1)
}

fn remove_org_subtree_dependency_in_lines(
    lines: &mut Vec<String>,
    source_line_number: usize,
    parent_line_number: usize,
) -> Result<usize> {
    let source_idx = line_index(source_line_number, "source task")?;
    let parent_idx = line_index(parent_line_number, "parent task")?;
    if parent_idx >= source_idx {
        anyhow::bail!("Dependency parent must appear before the task subtree.");
    }

    let source_level = heading_level_in_lines(lines, source_idx, source_line_number)?;
    let parent_level = heading_level_in_lines(lines, parent_idx, parent_line_number)?;
    if source_level <= parent_level {
        anyhow::bail!("Task is not nested under the dependency parent.");
    }
    let parent_end = heading_subtree_end_index(lines, parent_idx, parent_level);
    if source_idx >= parent_end {
        anyhow::bail!("Task is not nested under the dependency parent.");
    }

    let source_end = heading_subtree_end_index(lines, source_idx, source_level);
    let mut subtree = lines[source_idx..source_end].to_vec();
    relevel_subtree_lines(&mut subtree, parent_level, source_level)?;
    lines.drain(source_idx..source_end);

    let insert_idx = heading_subtree_end_index(lines, parent_idx, parent_level);
    lines.splice(insert_idx..insert_idx, subtree);
    Ok(insert_idx + 1)
}

fn relevel_subtree_lines(
    lines: &mut [String],
    new_root_level: usize,
    old_root_level: usize,
) -> Result<()> {
    let delta = new_root_level as isize - old_root_level as isize;
    for line in lines {
        let (body, newline) = crate::org_edit::split_line_ending(line);
        let Some(captures) = HEADING_RE.captures(body) else {
            continue;
        };
        let Some(stars) = captures.get(1) else {
            continue;
        };
        let new_level = stars.as_str().len() as isize + delta;
        if new_level < 1 {
            anyhow::bail!("Cannot move subtree because it would create an invalid heading level.");
        }
        let mut updated = body.to_string();
        updated.replace_range(stars.range(), &"*".repeat(new_level as usize));
        *line = format!("{updated}{newline}");
    }
    Ok(())
}

fn heading_level_in_content(content: &str, line_number: usize) -> Result<usize> {
    let line_idx = line_index(line_number, "task")?;
    let lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    heading_level_in_lines(&lines, line_idx, line_number)
}

fn heading_level_in_lines(lines: &[String], line_idx: usize, line_number: usize) -> Result<usize> {
    heading_level_at_index(lines, line_idx, line_number, "Task")
}

fn line_index(line_number: usize, name: &str) -> Result<usize> {
    line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid {name} line number: {line_number}"))
}

fn same_existing_path(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
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
    fn formats_typed_regular_note_task_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.org");
        std::fs::write(&path, "#+title: Inbox\n").unwrap();

        let line = append_org_task(
            &path,
            &OrgTaskInsertSpec {
                level: 1,
                state: OrgTodoState::new("TODO"),
                title: "Call Alice".to_string(),
                priority: Some(OrgPriority::A),
                tags: vec!["phone".to_string(), "urgent".to_string()],
                scheduled: Some("2026-05-27".to_string()),
                deadline: Some("2026-05-28 09:30".to_string()),
                project: Some("Focus".to_string()),
                description: Some("Discuss renewal".to_string()),
            },
        )
        .unwrap();

        assert_eq!(line, 3);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "#+title: Inbox\n\n* TODO [#A] Call Alice :phone:urgent:\nSCHEDULED: <2026-05-27 Wed> DEADLINE: <2026-05-28 Thu 09:30>\n:PROPERTIES:\n:PROJECT: Focus\n:END:\n\nDiscuss renewal\n"
        );
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
    fn inserts_typed_daily_inbox_task_inside_existing_inbox_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2026-05-27.org");
        std::fs::write(
            &path,
            "#+title: Day\n\n* Inbox\n** TODO Existing\n* Later\n",
        )
        .unwrap();

        let line = append_daily_inbox_task(
            &path,
            &OrgTaskInsertSpec {
                level: 2,
                state: OrgTodoState::new("TODO"),
                title: "New".to_string(),
                priority: None,
                tags: Vec::new(),
                scheduled: None,
                deadline: None,
                project: None,
                description: None,
            },
        )
        .unwrap();

        assert_eq!(line, 5);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
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
    fn creates_daily_note_with_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daily/2026-05-27.org");

        ensure_daily_note_exists(&OrgDailyNoteSpec {
            path: path.clone(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
        })
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            ":PROPERTIES:\n:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\n:END:\n#+title: 2026-05-27\n\n"
        );
    }

    #[test]
    fn keeps_existing_daily_note_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daily/2026-05-27.org");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "#+title: Existing\n\n* Inbox\n").unwrap();

        ensure_daily_note_exists(&OrgDailyNoteSpec {
            path: path.clone(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
        })
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "#+title: Existing\n\n* Inbox\n"
        );
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
    fn appends_typed_child_task_at_end_of_parent_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.org");
        std::fs::write(
            &path,
            "#+title: Tasks\n\n* Project\n** TODO Parent\nBody\n*** TODO Child\nChild body\n* TODO Sibling\n",
        )
        .unwrap();

        let line = append_child_org_task(
            &path,
            4,
            &OrgTaskInsertSpec {
                level: 3,
                state: OrgTodoState::new("TODO"),
                title: "New child".to_string(),
                priority: None,
                tags: Vec::new(),
                scheduled: None,
                deadline: None,
                project: None,
                description: None,
            },
        )
        .unwrap();

        assert_eq!(line, 8);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
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

    #[test]
    fn moves_subtree_to_end_of_target_subtree_and_relevels() {
        let mut lines = "#+title: Tasks\n\n* Project\n** TODO Target\n*** TODO Existing\n** TODO Source\nSource body\n*** TODO Child\n* TODO Sibling\n"
            .split_inclusive('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();

        let line = move_org_subtree_in_lines(&mut lines, 6, 4).unwrap();

        assert_eq!(line, 6);
        assert_eq!(
            lines.concat(),
            "#+title: Tasks\n\n* Project\n** TODO Target\n*** TODO Existing\n*** TODO Source\nSource body\n**** TODO Child\n* TODO Sibling\n"
        );
    }

    #[test]
    fn moves_subtree_before_later_target_and_adjusts_target_index() {
        let mut lines = "#+title: Tasks\n\n* Project\n** TODO Source\n*** TODO Child\n** TODO Middle\n** TODO Target\n*** TODO Existing\n* TODO Sibling\n"
            .split_inclusive('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();

        let line = move_org_subtree_in_lines(&mut lines, 4, 7).unwrap();

        assert_eq!(line, 7);
        assert_eq!(
            lines.concat(),
            "#+title: Tasks\n\n* Project\n** TODO Middle\n** TODO Target\n*** TODO Existing\n*** TODO Source\n**** TODO Child\n* TODO Sibling\n"
        );
    }

    #[test]
    fn rejects_moving_subtree_under_own_descendant() {
        let mut lines = "#+title: Tasks\n\n** TODO Source\n*** TODO Child\n"
            .split_inclusive('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();

        let err = move_org_subtree_in_lines(&mut lines, 3, 4).unwrap_err();

        assert!(
            err.to_string()
                .contains("Cannot move a task under one of its descendants")
        );
    }

    #[test]
    fn removes_subtree_dependency_by_moving_after_parent_subtree() {
        let mut lines = "#+title: Tasks\n\n* Project\n** TODO Parent\nParent body\n*** TODO Source\nSource body\n**** TODO Child\n** TODO Sibling\n"
            .split_inclusive('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();

        let line = remove_org_subtree_dependency_in_lines(&mut lines, 6, 4).unwrap();

        assert_eq!(line, 6);
        assert_eq!(
            lines.concat(),
            "#+title: Tasks\n\n* Project\n** TODO Parent\nParent body\n** TODO Source\nSource body\n*** TODO Child\n** TODO Sibling\n"
        );
    }
}
