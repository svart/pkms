use crate::org_date::parse_org_date;
use chrono::{Local, Timelike};
use std::collections::HashSet;

pub enum Filter {
    Include(String),
    Exclude(String),
}

pub fn parse_filters(s: Option<&str>) -> Vec<Filter> {
    match s {
        Some(s) => s
            .split(',')
            .map(|s| {
                let s = s.trim().to_string();
                if let Some(rest) = s.strip_prefix('!') {
                    Filter::Exclude(rest.to_string())
                } else {
                    Filter::Include(s)
                }
            })
            .collect(),
        None => Vec::new(),
    }
}

pub fn apply_state_filter(state: Option<&str>, filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                if !state.is_some_and(|s| s.eq_ignore_ascii_case(v)) {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                if state.is_some_and(|s| s.eq_ignore_ascii_case(v)) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn apply_tags_filter(tags: &[String], filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                if !tags.iter().any(|t| t == v) {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                if tags.iter().any(|t| t == v) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn apply_type_filter(has_scheduled: bool, has_deadline: bool, filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                let cond = match v.as_str() {
                    "SCHED" => has_scheduled,
                    "DEADL" => has_deadline,
                    _ => false,
                };
                if !cond {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                let cond = match v.as_str() {
                    "SCHED" => has_scheduled,
                    "DEADL" => has_deadline,
                    _ => false,
                };
                if cond {
                    return false;
                }
            }
        }
    }
    true
}

pub fn combine_tags(filetags: &[String], heading_tags: &[String]) -> String {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tag in filetags.iter().chain(heading_tags.iter()) {
        if seen.insert(tag.clone()) {
            result.push(tag.clone());
        }
    }
    result.join(", ")
}

pub fn extract_date(raw: Option<&String>) -> Option<String> {
    let raw = raw.as_ref()?;
    let parsed = parse_org_date(raw)?;
    Some(parsed.base_date.format("%Y-%m-%d").to_string())
}

pub fn is_overdue(raw: Option<&String>) -> bool {
    let raw = match raw {
        Some(r) => r,
        None => return false,
    };
    let parsed = match parse_org_date(raw) {
        Some(d) => d,
        None => return false,
    };
    let today = Local::now().date_naive();
    let compare_date = parsed.base_date_end.unwrap_or(parsed.base_date);
    if compare_date < today {
        return true;
    }
    if compare_date == today
        && let Some(et) = parsed.time_end
    {
        let now = Local::now().time();
        return now > et;
    }
    false
}

pub fn priority_value(p: char) -> u8 {
    match p {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
    }
}

pub fn format_display_datetime(raw: &str) -> String {
    let parsed = parse_org_date(raw);
    match parsed {
        Some(d) => {
            let date_str = d.base_date.format("%Y-%m-%d %a").to_string();
            let start_line = if let Some(t) = d.time {
                format!("{} {:02}:{:02}", date_str, t.hour(), t.minute())
            } else {
                date_str.clone()
            };
            if let Some(end_date) = d.base_date_end {
                let end_date_str = end_date.format("%Y-%m-%d %a").to_string();
                let end_line = if let Some(et) = d.time_end {
                    format!("{} {:02}:{:02}", end_date_str, et.hour(), et.minute())
                } else {
                    end_date_str
                };
                return format!("{}\n{}", start_line, end_line);
            }
            if let Some(et) = d.time_end {
                let padding = " ".repeat(date_str.len() + 1);
                return format!(
                    "{}\n{}{:02}:{:02}",
                    start_line,
                    padding,
                    et.hour(),
                    et.minute()
                );
            }
            start_line
        }
        None => raw.to_string(),
    }
}
