use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;

use crate::tasks::model::TaskItem;
use crate::tasks::scope::ResolvedScope;

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
    Any(Vec<TaskDateFilter>),
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

    pub fn matches_item(&self, item: &TaskItem, context: &TaskFilterContext<'_>) -> bool {
        matches_state_filter(
            item.state.as_deref(),
            self.state.as_deref(),
            context.open_todo_states,
            context.closed_todo_states,
        ) && matches_tags_filter(self.tags.as_deref(), &item.tags)
            && matches_type_filter(self.kind.as_deref(), item)
            && matches_priority_filter(self.prio.as_deref(), item)
            && self
                .date
                .as_ref()
                .is_none_or(|filter| filter.matches_item(item, context.today))
            && self
                .after
                .is_none_or(|after| item.datetimes().iter().any(|dt| dt >= &after))
            && self
                .before
                .is_none_or(|before| item.datetimes().iter().any(|dt| dt <= &before))
            && matches_project_filter(self.project.as_deref(), item)
            && matches_scope_filter(&self.scope, item, context.scope)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TaskFilterContext<'a> {
    pub today: NaiveDate,
    pub scope: Option<&'a ResolvedScope>,
    pub open_todo_states: &'a [String],
    pub closed_todo_states: &'a [String],
}

impl TaskDateFilter {
    pub fn matches_item(&self, item: &TaskItem, today: NaiveDate) -> bool {
        match self {
            TaskDateFilter::Exact(date) => item.dates().iter().any(|item_date| item_date == date),
            TaskDateFilter::Today => item.dates().contains(&today),
            TaskDateFilter::Week => {
                let cutoff = today + Duration::days(7);
                item.dates().iter().any(|item_date| *item_date <= cutoff)
            }
            TaskDateFilter::Overdue => item.is_overdue,
            TaskDateFilter::Upcoming => {
                !item.is_overdue && item.dates().iter().any(|item_date| *item_date > today)
            }
            TaskDateFilter::Any(filters) => filters
                .iter()
                .any(|filter| filter.matches_item(item, today)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextFilter {
    Include(String),
    Exclude(String),
}

pub fn parse_text_filters(value: Option<&str>) -> Vec<TextFilter> {
    value
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    part.strip_prefix('!')
                        .map(|part| TextFilter::Exclude(part.to_string()))
                        .unwrap_or_else(|| TextFilter::Include(part.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn matches_text_filters(value: Option<&str>, filters: &[TextFilter]) -> bool {
    filters.iter().all(|filter| match filter {
        TextFilter::Include(target) => {
            value.is_some_and(|value| value.eq_ignore_ascii_case(target))
        }
        TextFilter::Exclude(target) => {
            !value.is_some_and(|value| value.eq_ignore_ascii_case(target))
        }
    })
}

pub fn matches_tag_filters(tags: &[String], filters: &[TextFilter]) -> bool {
    filters.iter().all(|filter| match filter {
        TextFilter::Include(target) => tags.iter().any(|tag| tag == target),
        TextFilter::Exclude(target) => !tags.iter().any(|tag| tag == target),
    })
}

pub fn matches_type_filters(
    has_scheduled: bool,
    has_deadline: bool,
    filters: &[TextFilter],
) -> bool {
    filters.iter().all(|filter| {
        let matched = match filter {
            TextFilter::Include(target) | TextFilter::Exclude(target) => match target.as_str() {
                "SCHED" => has_scheduled,
                "DEADL" => has_deadline,
                _ => false,
            },
        };
        matches!(filter, TextFilter::Include(_)) == matched
    })
}

fn matches_state_filter(
    value: Option<&str>,
    filters: Option<&str>,
    open_todo_states: &[String],
    closed_todo_states: &[String],
) -> bool {
    parse_text_filters(filters)
        .iter()
        .all(|filter| match filter {
            TextFilter::Include(target) => {
                state_matches_target(value, target, open_todo_states, closed_todo_states)
            }
            TextFilter::Exclude(target) => {
                !state_matches_target(value, target, open_todo_states, closed_todo_states)
            }
        })
}

fn state_matches_target(
    value: Option<&str>,
    target: &str,
    open_todo_states: &[String],
    closed_todo_states: &[String],
) -> bool {
    let Some(value) = value else {
        return false;
    };
    match target.to_ascii_lowercase().as_str() {
        "opened" => state_in_configured_set(value, open_todo_states),
        "closed" => state_in_configured_set(value, closed_todo_states),
        _ => value.eq_ignore_ascii_case(target),
    }
}

fn state_in_configured_set(value: &str, states: &[String]) -> bool {
    states.iter().any(|state| state.eq_ignore_ascii_case(value))
}

fn matches_tags_filter(filters: Option<&str>, tags: &[String]) -> bool {
    matches_tag_filters(tags, &parse_text_filters(filters))
}

fn matches_type_filter(filters: Option<&str>, item: &TaskItem) -> bool {
    matches_type_filters(
        item.scheduled.is_some(),
        item.deadline.is_some(),
        &parse_text_filters(filters),
    )
}

fn matches_priority_filter(filter: Option<&str>, item: &TaskItem) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    if filter.is_empty() {
        return item.priority.is_none();
    }
    let targets = priority_filter_targets(filter);
    priority_matches_target(item.priority_char(), &targets)
}

pub fn priority_filter_targets(prio: &str) -> Vec<char> {
    prio.split(',')
        .filter_map(|value| value.trim().chars().next())
        .map(|priority| priority.to_ascii_uppercase())
        .collect()
}

pub fn priority_matches_target(priority: Option<char>, targets: &[char]) -> bool {
    priority.is_some_and(|priority| targets.contains(&priority.to_ascii_uppercase()))
}

fn matches_project_filter(filters: Option<&str>, item: &TaskItem) -> bool {
    parse_text_filters(filters)
        .iter()
        .all(|filter| match filter {
            TextFilter::Include(target) => task_item_project_matches(item, target),
            TextFilter::Exclude(target) => !task_item_project_matches(item, target),
        })
}

fn task_item_project_matches(item: &TaskItem, value: &str) -> bool {
    item.project
        .as_deref()
        .is_some_and(|project| project.eq_ignore_ascii_case(value))
        || item
            .project_id
            .as_deref()
            .is_some_and(|project_id| project_id.eq_ignore_ascii_case(value))
}

fn matches_scope_filter(
    raw_scope: &[String],
    item: &TaskItem,
    scope: Option<&ResolvedScope>,
) -> bool {
    raw_scope.is_empty() || scope.is_some_and(|scope| scope.matches_task_item(item))
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
        value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_uppercase)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn parse_date_filter(value: &str) -> Result<TaskDateFilter> {
    let values: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if values.len() > 1 {
        return values
            .into_iter()
            .map(parse_single_date_filter)
            .collect::<Result<Vec<_>>>()
            .map(TaskDateFilter::Any);
    }
    parse_single_date_filter(value.trim())
}

fn parse_single_date_filter(value: &str) -> Result<TaskDateFilter> {
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
    fn parses_comma_separated_priority_filters() {
        let filters = parse_task_filters(&["prio:a,b,c".to_string()]).unwrap();
        assert_eq!(filters.criteria.prio.as_deref(), Some("A,B,C"));
    }

    #[test]
    fn matches_state_meta_filters_against_configured_states() {
        let open_states = vec!["TODO".to_string(), "WAITING".to_string()];
        let closed_states = vec!["DONE".to_string(), "CANCELED".to_string()];

        assert!(matches_state_filter(
            Some("TODO"),
            Some("opened"),
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter(
            Some("DONE"),
            Some("closed"),
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter(
            Some("TODO"),
            Some("!closed"),
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter(
            Some("TODO"),
            Some("opened,!waiting"),
            &open_states,
            &closed_states
        ));
        assert!(!matches_state_filter(
            Some("WAITING"),
            Some("opened,!waiting"),
            &open_states,
            &closed_states
        ));
        assert!(!matches_state_filter(
            Some("DONE"),
            Some("opened"),
            &open_states,
            &closed_states
        ));
    }

    #[test]
    fn parses_comma_separated_date_filters() {
        let filters = parse_task_filters(&["date:today,2026-05-10,upcoming".to_string()]).unwrap();
        assert_eq!(
            filters.criteria.date,
            Some(TaskDateFilter::Any(vec![
                TaskDateFilter::Today,
                TaskDateFilter::Exact(NaiveDate::from_ymd_opt(2026, 5, 10).unwrap()),
                TaskDateFilter::Upcoming,
            ]))
        );
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
