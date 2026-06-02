use crate::org_edit;
use crate::parser::{DEADLINE_RE, HEADING_RE, SCHEDULED_RE};
use anyhow::{Result, bail};
use chrono::{NaiveDate, NaiveDateTime};

#[derive(Debug, Clone, Copy)]
pub enum PlanningKind {
    Scheduled,
    Deadline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStateChange {
    pub path: String,
    pub line_number: usize,
    pub old_state: String,
    pub new_state: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingMod {
    pub title: Option<String>,
    pub priority: Option<Option<char>>,
    pub tags: Option<Vec<String>>,
    pub scheduled: Option<Option<String>>,
    pub deadline: Option<Option<String>>,
    pub project: Option<Option<String>>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPropertyChange {
    pub property: &'static str,
    pub old: Option<String>,
    pub new: Option<String>,
}

pub fn replace_heading_state(
    path: &str,
    line_number: usize,
    new_state: &str,
    dry_run: bool,
) -> Result<TaskStateChange> {
    let mut lines = org_edit::read_lines(path)?;
    let idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let line = lines
        .get(idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?
        .clone();
    let (body, newline) = org_edit::split_line_ending(&line);
    let captures = HEADING_RE
        .captures(body)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} is no longer an org heading"))?;
    let state_match = captures
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} does not have a TODO state"))?;
    let old_state = state_match.as_str().to_string();
    if old_state == new_state {
        return Ok(TaskStateChange {
            path: path.to_string(),
            line_number,
            old_state,
            new_state: new_state.to_string(),
            dry_run,
        });
    }

    let mut updated = body.to_string();
    updated.replace_range(state_match.range(), new_state);
    lines[idx] = format!("{updated}{newline}");
    if !dry_run {
        org_edit::write_lines(path, &lines)?;
    }

    Ok(TaskStateChange {
        path: path.to_string(),
        line_number,
        old_state,
        new_state: new_state.to_string(),
        dry_run,
    })
}

pub fn update_heading_properties(
    path: &str,
    line_number: usize,
    modifier: &HeadingMod,
) -> Result<Vec<TaskPropertyChange>> {
    let mut lines = org_edit::read_lines(path)?;
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let heading = lines
        .get(heading_idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?
        .clone();
    let (heading_body, heading_newline) = org_edit::split_line_ending(&heading);
    let captures = HEADING_RE
        .captures(heading_body)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} is no longer an org heading"))?;

    let mut changes = Vec::new();
    let level = captures.get(1).map_or("", |m| m.as_str());
    let state = captures.get(2).map(|m| m.as_str().to_string());
    let old_priority = captures.get(3).and_then(|m| m.as_str().chars().next());
    let old_title = captures
        .get(4)
        .map_or("", |m| m.as_str())
        .trim()
        .to_string();
    let old_tags = heading_tags(captures.get(5).map(|m| m.as_str()));

    let new_title = modifier.title.as_ref().unwrap_or(&old_title);
    let new_priority = modifier.priority.unwrap_or(old_priority);
    let new_tags = modifier.tags.as_ref().unwrap_or(&old_tags);

    if modifier.title.is_some() && old_title != *new_title {
        changes.push(TaskPropertyChange {
            property: "Title",
            old: Some(old_title.clone()),
            new: Some(new_title.clone()),
        });
    }
    if modifier.priority.is_some() && old_priority != new_priority {
        changes.push(TaskPropertyChange {
            property: "Priority",
            old: old_priority.map(|p| p.to_string()),
            new: new_priority.map(|p| p.to_string()),
        });
    }
    if modifier.tags.is_some() && old_tags != *new_tags {
        changes.push(TaskPropertyChange {
            property: "Tags",
            old: non_empty_tags(&old_tags),
            new: non_empty_tags(new_tags),
        });
    }
    if changes
        .iter()
        .any(|change| matches!(change.property, "Title" | "Priority" | "Tags"))
    {
        lines[heading_idx] = format!(
            "{}{}",
            format_heading(level, state.as_deref(), new_priority, new_title, new_tags),
            heading_newline
        );
    }

    apply_planning_change(
        &mut lines,
        heading_idx,
        PlanningKind::Scheduled,
        modifier.scheduled.as_ref(),
        &mut changes,
    )?;
    apply_planning_change(
        &mut lines,
        heading_idx,
        PlanningKind::Deadline,
        modifier.deadline.as_ref(),
        &mut changes,
    )?;
    apply_project_change(
        &mut lines,
        heading_idx,
        modifier.project.as_ref(),
        &mut changes,
    );
    apply_description_change(
        &mut lines,
        heading_idx,
        modifier.description.as_ref(),
        &mut changes,
    );

    if !changes.is_empty() {
        org_edit::write_lines(path, &lines)?;
    }
    Ok(changes)
}

pub fn update_heading_planning_date(
    path: &str,
    line_number: usize,
    kind: PlanningKind,
    date: Option<&str>,
) -> Result<()> {
    let mut lines = org_edit::read_lines(path)?;
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let heading = lines
        .get(heading_idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?;
    if !HEADING_RE.is_match(heading.trim_end()) {
        bail!("Task line {line_number} is no longer an org heading");
    }

    let planning_idx = find_planning_line_index(&lines, heading_idx);
    match (planning_idx, date) {
        (Some(idx), Some(date)) => {
            let updated = replace_planning_token(&lines[idx], kind, Some(&org_date(date)?));
            lines[idx] = updated;
        }
        (Some(idx), None) => {
            let updated = replace_planning_token(&lines[idx], kind, None);
            if updated.trim().is_empty() {
                lines.remove(idx);
            } else {
                lines[idx] = updated;
            }
        }
        (None, Some(date)) => {
            lines.insert(
                heading_idx + 1,
                format!("{}: {}\n", planning_label(kind), org_date(date)?),
            );
        }
        (None, None) => {}
    }
    org_edit::write_lines(path, &lines)?;
    Ok(())
}

fn heading_tags(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(':')
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn non_empty_tags(tags: &[String]) -> Option<String> {
    (!tags.is_empty()).then(|| tags.join(", "))
}

fn format_heading(
    level: &str,
    state: Option<&str>,
    priority: Option<char>,
    title: &str,
    tags: &[String],
) -> String {
    let mut parts = vec![level.to_string()];
    if let Some(state) = state {
        parts.push(state.to_string());
    }
    if let Some(priority) = priority {
        parts.push(format!("[#{priority}]"));
    }
    parts.push(title.trim().to_string());
    let mut heading = parts.join(" ");
    if !tags.is_empty() {
        heading.push(' ');
        heading.push(':');
        heading.push_str(&tags.join(":"));
        heading.push(':');
    }
    heading
}

fn apply_planning_change(
    lines: &mut Vec<String>,
    heading_idx: usize,
    kind: PlanningKind,
    requested: Option<&Option<String>>,
    changes: &mut Vec<TaskPropertyChange>,
) -> Result<()> {
    let Some(requested) = requested else {
        return Ok(());
    };
    let old = current_planning_value(lines, heading_idx, kind);
    let new = requested.as_deref().map(org_date).transpose()?;
    if old == new {
        return Ok(());
    }

    let planning_idx = find_planning_line_index(lines, heading_idx);
    match (planning_idx, new.as_deref()) {
        (Some(idx), Some(value)) => {
            lines[idx] = replace_planning_token(&lines[idx], kind, Some(value));
        }
        (Some(idx), None) => {
            let updated = replace_planning_token(&lines[idx], kind, None);
            if updated.trim().is_empty() {
                lines.remove(idx);
            } else {
                lines[idx] = updated;
            }
        }
        (None, Some(value)) => {
            lines.insert(
                heading_idx + 1,
                format!("{}: {value}\n", planning_label(kind)),
            );
        }
        (None, None) => {}
    }
    changes.push(TaskPropertyChange {
        property: planning_display_label(kind),
        old,
        new,
    });
    Ok(())
}

fn current_planning_value(
    lines: &[String],
    heading_idx: usize,
    kind: PlanningKind,
) -> Option<String> {
    let idx = find_planning_line_index(lines, heading_idx)?;
    let regex = match kind {
        PlanningKind::Scheduled => &*SCHEDULED_RE,
        PlanningKind::Deadline => &*DEADLINE_RE,
    };
    regex
        .captures(lines[idx].trim_end())
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

fn planning_display_label(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Scheduled => "Scheduled",
        PlanningKind::Deadline => "Deadline",
    }
}

fn apply_project_change(
    lines: &mut Vec<String>,
    heading_idx: usize,
    requested: Option<&Option<String>>,
    changes: &mut Vec<TaskPropertyChange>,
) {
    let Some(requested) = requested else {
        return;
    };
    let old = current_heading_project(lines, heading_idx);
    if old.as_ref() == requested.as_ref() {
        return;
    }

    set_heading_project(lines, heading_idx, requested.as_deref());
    changes.push(TaskPropertyChange {
        property: "Project",
        old,
        new: requested.clone(),
    });
}

fn current_heading_project(lines: &[String], heading_idx: usize) -> Option<String> {
    let (start, end) = find_property_drawer(lines, heading_idx)?;
    lines[start + 1..end].iter().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(":PROJECT:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn set_heading_project(lines: &mut Vec<String>, heading_idx: usize, value: Option<&str>) {
    if let Some((start, end)) = find_property_drawer(lines, heading_idx) {
        if let Some(project_idx) =
            (start + 1..end).find(|idx| lines[*idx].trim_start().starts_with(":PROJECT:"))
        {
            if let Some(value) = value {
                lines[project_idx] = format!(":PROJECT: {value}\n");
            } else {
                lines.remove(project_idx);
                remove_empty_property_drawer(lines, start);
            }
            return;
        }
        if let Some(value) = value {
            lines.insert(end, format!(":PROJECT: {value}\n"));
        }
        return;
    }

    if let Some(value) = value {
        let insert_idx = metadata_insert_index(lines, heading_idx);
        lines.insert(insert_idx, ":PROPERTIES:\n".to_string());
        lines.insert(insert_idx + 1, format!(":PROJECT: {value}\n"));
        lines.insert(insert_idx + 2, ":END:\n".to_string());
    }
}

fn remove_empty_property_drawer(lines: &mut Vec<String>, start: usize) {
    if start + 1 < lines.len() && lines[start + 1].trim() == ":END:" {
        lines.remove(start + 1);
        lines.remove(start);
    }
}

fn find_property_drawer(lines: &[String], heading_idx: usize) -> Option<(usize, usize)> {
    let mut idx = heading_idx + 1;
    if let Some(planning_idx) = find_planning_line_index(lines, heading_idx)
        && planning_idx == idx
    {
        idx += 1;
    }
    while idx < lines.len() && lines[idx].trim().is_empty() {
        idx += 1;
    }
    if lines
        .get(idx)
        .is_none_or(|line| line.trim() != ":PROPERTIES:")
    {
        return None;
    }
    for (end, line) in lines.iter().enumerate().skip(idx + 1) {
        if line.trim() == ":END:" {
            return Some((idx, end));
        }
        if HEADING_RE.is_match(line.trim_end()) {
            return None;
        }
    }
    None
}

fn metadata_insert_index(lines: &[String], heading_idx: usize) -> usize {
    find_planning_line_index(lines, heading_idx)
        .map(|idx| idx + 1)
        .unwrap_or(heading_idx + 1)
}

fn apply_description_change(
    lines: &mut Vec<String>,
    heading_idx: usize,
    requested: Option<&String>,
    changes: &mut Vec<TaskPropertyChange>,
) {
    let Some(requested) = requested else {
        return;
    };
    let (start, end) = description_range(lines, heading_idx);
    let old = lines[start..end]
        .iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let old = (!old.is_empty()).then_some(old);
    let new = (!requested.trim().is_empty()).then(|| requested.trim().to_string());
    if old == new {
        return;
    }

    lines.drain(start..end);
    if let Some(new) = &new {
        lines.insert(start, "\n".to_string());
        for (offset, line) in new.lines().enumerate() {
            lines.insert(start + 1 + offset, format!("{line}\n"));
        }
    }
    changes.push(TaskPropertyChange {
        property: "Description",
        old,
        new,
    });
}

fn description_range(lines: &[String], heading_idx: usize) -> (usize, usize) {
    let mut start = heading_idx + 1;
    if let Some(planning_idx) = find_planning_line_index(lines, heading_idx)
        && planning_idx == start
    {
        start += 1;
    }
    if let Some((drawer_start, drawer_end)) = find_property_drawer(lines, heading_idx)
        && drawer_start >= start
    {
        start = drawer_end + 1;
    }
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }
    let mut end = start;
    while end < lines.len() {
        if HEADING_RE.is_match(lines[end].trim_end()) {
            break;
        }
        end += 1;
    }
    (start, end)
}

pub fn update_recurring_planning_date(path: &str, line_number: usize, date: &str) -> Result<()> {
    let new_date = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    let mut lines = org_edit::read_lines(path)?;
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let planning_idx = find_planning_line_index(&lines, heading_idx).ok_or_else(|| {
        anyhow::anyhow!("Task does not have a recurring scheduled or deadline date")
    })?;
    lines[planning_idx] = postpone_recurring_planning_line(&lines[planning_idx], new_date)?;
    org_edit::write_lines(path, &lines)?;
    Ok(())
}

fn find_planning_line_index(lines: &[String], heading_idx: usize) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(heading_idx + 1) {
        let body = line.trim_end();
        if HEADING_RE.is_match(body) {
            return None;
        }
        if body.trim().is_empty() {
            continue;
        }
        if SCHEDULED_RE.is_match(body) || DEADLINE_RE.is_match(body) {
            return Some(idx);
        }
        return None;
    }
    None
}

fn postpone_recurring_planning_line(line: &str, new_date: NaiveDate) -> Result<String> {
    if let Some(updated) = postpone_recurring_token(line, &SCHEDULED_RE, "SCHEDULED", new_date)? {
        return Ok(updated);
    }
    if let Some(updated) = postpone_recurring_token(line, &DEADLINE_RE, "DEADLINE", new_date)? {
        return Ok(updated);
    }
    bail!("Task does not have a recurring scheduled or deadline date")
}

fn postpone_recurring_token(
    line: &str,
    regex: &regex::Regex,
    label: &str,
    new_date: NaiveDate,
) -> Result<Option<String>> {
    let Some(captures) = regex.captures(line) else {
        return Ok(None);
    };
    let Some(raw_match) = captures.get(1) else {
        return Ok(None);
    };
    let parsed = crate::org_date::parse_org_date(raw_match.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not parse existing {label} date"))?;
    if parsed.repeater.is_none() {
        bail!("Task {label} date is not recurring");
    }
    let replacement = format!("{label}: {}", format_org_date_like(&parsed, new_date));
    Ok(Some(regex.replace(line, replacement.as_str()).to_string()))
}

fn format_org_date_like(existing: &crate::org_date::OrgDate, new_date: NaiveDate) -> String {
    let open = if existing.inactive { "[" } else { "<" };
    let close = if existing.inactive { "]" } else { ">" };
    let mut parts = vec![new_date.format("%Y-%m-%d %a").to_string()];
    if let Some(time) = existing.time {
        let mut time_part = time.format("%H:%M").to_string();
        if let Some(end) = existing.time_end {
            time_part.push('-');
            time_part.push_str(&end.format("%H:%M").to_string());
        }
        parts.push(time_part);
    }
    if let Some(repeater) = existing.repeater.as_deref() {
        parts.push(repeater.to_string());
    }
    if let Some(warning) = existing.warning.as_deref() {
        parts.push(warning.to_string());
    }
    format!("{open}{}{close}", parts.join(" "))
}

fn planning_label(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Scheduled => "SCHEDULED",
        PlanningKind::Deadline => "DEADLINE",
    }
}

fn replace_planning_token(line: &str, kind: PlanningKind, value: Option<&str>) -> String {
    let (body, newline) = org_edit::split_line_ending(line);
    let regex = match kind {
        PlanningKind::Scheduled => &*SCHEDULED_RE,
        PlanningKind::Deadline => &*DEADLINE_RE,
    };
    let label = planning_label(kind);
    let updated = if regex.is_match(body) {
        match value {
            Some(value) => regex
                .replace(body, format!("{label}: {value}").as_str())
                .to_string(),
            None => regex.replace(body, "").to_string(),
        }
    } else {
        match value {
            Some(value) if body.trim().is_empty() => format!("{label}: {value}"),
            Some(value) => format!("{} {label}: {value}", body.trim_end()),
            None => body.to_string(),
        }
    };
    format!("{}{}", updated.trim(), newline)
}

fn org_date(date: &str) -> Result<String> {
    if let Ok(datetime) = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M") {
        return Ok(format!("<{}>", datetime.format("%Y-%m-%d %a %H:%M")));
    }
    Ok(format!(
        "<{}>",
        NaiveDate::parse_from_str(date, "%Y-%m-%d")?.format("%Y-%m-%d %a")
    ))
}
