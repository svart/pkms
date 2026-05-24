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

        bail!("Task filter '{filter}' is not implemented yet");
    }

    let source = normalize_sources(selected);
    if todoist_filter.is_some() && matches!(source, SourceSelection::Pkms) {
        bail!("todoist.filter requires source:todoist or source:all");
    }

    Ok(TaskFilters {
        source,
        todoist_filter,
        criteria: TaskFilterCriteria::default(),
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
        assert!(parse_source_selection(&["state:TODO".to_string()]).is_err());
        assert!(parse_source_selection(&["source:remote".to_string()]).is_err());
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
