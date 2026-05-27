use crate::org_edit;
use crate::parser::{DEADLINE_RE, HEADING_RE, SCHEDULED_RE};
use anyhow::{Result, bail};
use chrono::NaiveDate;

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
    Ok(format!(
        "<{}>",
        NaiveDate::parse_from_str(date, "%Y-%m-%d")?.format("%Y-%m-%d %a")
    ))
}
