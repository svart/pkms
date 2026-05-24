use anyhow::{Result, bail};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceSelection {
    Pkms,
    Todoist,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFilters {
    pub source: SourceSelection,
    pub todoist_filter: Option<String>,
    pub criteria: TaskFilterCriteria,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFilterCriteria {
    pub state: Option<String>,
    pub tags: Option<String>,
    pub kind: Option<String>,
    pub prio: Option<String>,
    pub date: Option<TaskDateFilter>,
    pub after: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub scope: Vec<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDateFilter {
    Exact(NaiveDate),
    Today,
    Week,
    Overdue,
    Upcoming,
}

pub fn parse_source_selection(filters: &[String]) -> Result<SourceSelection> {
    Ok(parse_task_filters(filters)?.source)
}

pub fn parse_task_filters(filters: &[String]) -> Result<TaskFilters> {
    let mut selected = Vec::new();
    let mut todoist_filter = None;
    let mut criteria = TaskFilterCriteria::default();
    for filter in filters {
        if let Some(value) = filter
            .strip_prefix("source:")
            .or_else(|| filter.strip_prefix("src:"))
        {
            match value.to_ascii_lowercase().as_str() {
                "pkms" => selected.push(SourceSelection::Pkms),
                "todoist" => selected.push(SourceSelection::Todoist),
                "all" => selected.push(SourceSelection::All),
                _ => bail!(
                    "Unknown task source '{value}'. Use source:pkms, source:todoist, or source:all."
                ),
            }
            continue;
        }

        if let Some(value) = filter.strip_prefix("todoist.filter:") {
            todoist_filter = Some(unquote(value).to_string());
            continue;
        }

        if matches!(filter.as_str(), "overdue" | "upcoming") {
            criteria.date = Some(match filter.as_str() {
                "overdue" => TaskDateFilter::Overdue,
                "upcoming" => TaskDateFilter::Upcoming,
                _ => unreachable!(),
            });
            continue;
        }

        if let Some((key, value)) = filter.split_once(':') {
            let value = unquote(value).to_string();
            match key {
                "state" => criteria.state = Some(value),
                "tags" | "tag" => criteria.tags = Some(value),
                "type" | "kind" => criteria.kind = Some(value.to_ascii_uppercase()),
                "prio" | "priority" => criteria.prio = Some(parse_priority_filter(&value)),
                "date" => criteria.date = Some(parse_date_filter(&value)?),
                "after" => criteria.after = Some(parse_datetime_filter(&value)?),
                "before" => criteria.before = Some(parse_datetime_filter(&value)?),
                "scope" => criteria.scope.push(value),
                "project" => criteria.project = Some(value),
                _ => bail!("Task filter '{filter}' is not implemented yet"),
            }
            continue;
        }

        bail!("Task filter '{filter}' is not implemented yet");
    }

    let source = normalize_sources(selected);
    if todoist_filter.is_some() && matches!(source, SourceSelection::Pkms) {
        bail!("todoist.filter requires source:todoist or source:all");
    }

    Ok(TaskFilters {
        source,
        todoist_filter,
        criteria,
    })
}

impl TaskFilters {
    pub fn with_todoist_filter(&self, todoist_filter: Option<String>) -> Self {
        Self {
            source: self.source,
            todoist_filter,
            criteria: self.criteria.clone(),
        }
    }

    pub fn has_criteria(&self) -> bool {
        self.criteria.has_filters()
    }
}

impl TaskFilterCriteria {
    pub fn has_filters(&self) -> bool {
        self.state.is_some()
            || self.tags.is_some()
            || self.kind.is_some()
            || self.prio.is_some()
            || self.date.is_some()
            || self.after.is_some()
            || self.before.is_some()
            || !self.scope.is_empty()
            || self.project.is_some()
    }
}

fn normalize_sources(mut selected: Vec<SourceSelection>) -> SourceSelection {
    if selected.is_empty() {
        return SourceSelection::Pkms;
    }

    if selected.contains(&SourceSelection::All) {
        return SourceSelection::All;
    }

    selected.sort_by_key(|source| match source {
        SourceSelection::Pkms => 0,
        SourceSelection::Todoist => 1,
        SourceSelection::All => 2,
    });
    selected.dedup();

    match selected.as_slice() {
        [SourceSelection::Pkms] => SourceSelection::Pkms,
        [SourceSelection::Todoist] => SourceSelection::Todoist,
        _ => SourceSelection::All,
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

fn parse_priority_filter(value: &str) -> String {
    if value.eq_ignore_ascii_case("none") {
        String::new()
    } else {
        value.to_ascii_uppercase()
    }
}

fn parse_date_filter(value: &str) -> Result<TaskDateFilter> {
    match value {
        "today" => Ok(TaskDateFilter::Today),
        "week" => Ok(TaskDateFilter::Week),
        "overdue" => Ok(TaskDateFilter::Overdue),
        "upcoming" => Ok(TaskDateFilter::Upcoming),
        _ => Ok(TaskDateFilter::Exact(parse_date(value)?)),
    }
}

fn parse_datetime_filter(value: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
        .or_else(|_| parse_date(value).map(|date| date.and_hms_opt(0, 0, 0).unwrap()))
        .map_err(|_| {
            anyhow::anyhow!("Invalid date/time '{value}'. Use YYYY-MM-DD or YYYY-MM-DD HH:MM.")
        })
}

fn parse_date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Invalid date '{value}'. Use YYYY-MM-DD."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_pkms_source() {
        assert_eq!(parse_source_selection(&[]).unwrap(), SourceSelection::Pkms);
    }

    #[test]
    fn parses_source_aliases() {
        assert_eq!(
            parse_source_selection(&["src:pkms".to_string()]).unwrap(),
            SourceSelection::Pkms
        );
        assert_eq!(
            parse_source_selection(&["source:todoist".to_string()]).unwrap(),
            SourceSelection::Todoist
        );
    }

    #[test]
    fn repeated_sources_combine_to_all() {
        assert_eq!(
            parse_source_selection(&["source:pkms".to_string(), "source:todoist".to_string()])
                .unwrap(),
            SourceSelection::All
        );
    }

    #[test]
    fn rejects_unknown_filters_initially() {
        assert!(parse_source_selection(&["unknown:value".to_string()]).is_err());
        assert!(parse_source_selection(&["source:remote".to_string()]).is_err());
    }

    #[test]
    fn parses_todo_agenda_criteria_filters() {
        let filters = parse_task_filters(&[
            "state:TODO,!WAITING".to_string(),
            "tags:!tag1,tag2".to_string(),
            "type:SCHED".to_string(),
            "prio:none".to_string(),
            "date:week".to_string(),
            "after:2026-05-01".to_string(),
            "before:2026-05-25 18:00".to_string(),
            "scope:Project Note".to_string(),
            "project:Alpha".to_string(),
        ])
        .unwrap();
        assert_eq!(filters.criteria.state.as_deref(), Some("TODO,!WAITING"));
        assert_eq!(filters.criteria.tags.as_deref(), Some("!tag1,tag2"));
        assert_eq!(filters.criteria.kind.as_deref(), Some("SCHED"));
        assert_eq!(filters.criteria.prio.as_deref(), Some(""));
        assert_eq!(filters.criteria.date, Some(TaskDateFilter::Week));
        assert!(filters.criteria.after.is_some());
        assert!(filters.criteria.before.is_some());
        assert_eq!(filters.criteria.scope, vec!["Project Note"]);
        assert_eq!(filters.criteria.project.as_deref(), Some("Alpha"));
    }

    #[test]
    fn parses_todoist_filter() {
        let filters = parse_task_filters(&[
            "source:todoist".to_string(),
            "todoist.filter:\"today | overdue\"".to_string(),
        ])
        .unwrap();
        assert_eq!(filters.source, SourceSelection::Todoist);
        assert_eq!(filters.todoist_filter.as_deref(), Some("today | overdue"));
    }

    #[test]
    fn rejects_todoist_filter_for_pkms_source() {
        assert!(parse_task_filters(&["todoist.filter:today".to_string()]).is_err());
        assert!(
            parse_task_filters(&[
                "source:pkms".to_string(),
                "todoist.filter:today".to_string()
            ])
            .is_err()
        );
    }
}
