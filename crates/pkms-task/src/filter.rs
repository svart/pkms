use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;

#[cfg(test)]
use crate::clock::TaskClock;
use crate::model::TaskItem;
use crate::modifiers::parse_task_date_arg_on;
use crate::scope::ResolvedScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceSelection {
    Pkms,
    Todoist,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFilters {
    pub(crate) source: SourceSelection,
    pub(crate) todoist_filter: Option<String>,
    pub(crate) criteria: TaskFilterCriteria,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFilterCriteria {
    pub state: Option<TextFilterCriteria>,
    pub tags: Option<TextFilterCriteria>,
    pub kind: Option<TextFilterCriteria>,
    pub prio: Option<PriorityFilter>,
    pub date: Option<TaskDateFilter>,
    pub after: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub scope: Vec<String>,
    pub project: Option<TextFilterCriteria>,
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

#[cfg(test)]
fn parse_task_filters(filters: &[String]) -> Result<TaskFilters> {
    parse_task_filters_on(filters, TaskClock::now().today)
}

pub fn parse_task_filters_on(filters: &[String], today: NaiveDate) -> Result<TaskFilters> {
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
                "state" => criteria.state = Some(TextFilterCriteria::parse(&value)),
                "tags" | "tag" => criteria.tags = Some(TextFilterCriteria::parse(&value)),
                "type" | "kind" => {
                    criteria.kind = Some(TextFilterCriteria::parse(&value.to_ascii_uppercase()))
                }
                "prio" | "priority" => criteria.prio = Some(parse_priority_filter(&value)),
                "date" => criteria.date = Some(parse_date_filter(&value, today)?),
                "after" => criteria.after = Some(parse_datetime_filter("after", &value, today)?),
                "before" => criteria.before = Some(parse_datetime_filter("before", &value, today)?),
                "scope" => criteria.scope.push(value),
                "project" => criteria.project = Some(TextFilterCriteria::parse(&value)),
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
    pub fn source(&self) -> SourceSelection {
        self.source
    }

    pub fn todoist_filter(&self) -> Option<&str> {
        self.todoist_filter.as_deref()
    }

    pub fn scope(&self) -> &[String] {
        &self.criteria.scope
    }

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
            self.state.as_ref(),
            context.open_todo_states,
            context.closed_todo_states,
        ) && matches_tags_filter(self.tags.as_ref(), &item.tags)
            && matches_type_filter(self.kind.as_ref(), item)
            && matches_priority_filter(self.prio.as_ref(), item)
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
            && matches_project_filter(self.project.as_ref(), item)
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
            TaskDateFilter::Overdue => item.is_overdue_on(today),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFilterCriteria {
    filters: Vec<TextFilter>,
}

impl TextFilterCriteria {
    pub fn parse(value: &str) -> Self {
        Self {
            filters: parse_text_filters(Some(value)),
        }
    }

    pub fn filters(&self) -> &[TextFilter] {
        &self.filters
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriorityFilter {
    NoPriority,
    Targets(Vec<char>),
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
    filters: Option<&TextFilterCriteria>,
    open_todo_states: &[String],
    closed_todo_states: &[String],
) -> bool {
    filters
        .map(TextFilterCriteria::filters)
        .unwrap_or_default()
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

fn matches_tags_filter(filters: Option<&TextFilterCriteria>, tags: &[String]) -> bool {
    filters.is_none_or(|filters| matches_tag_filters(tags, filters.filters()))
}

fn matches_type_filter(filters: Option<&TextFilterCriteria>, item: &TaskItem) -> bool {
    filters.is_none_or(|filters| {
        matches_type_filters(
            item.scheduled.is_some(),
            item.deadline.is_some(),
            filters.filters(),
        )
    })
}

fn matches_priority_filter(filter: Option<&PriorityFilter>, item: &TaskItem) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    match filter {
        PriorityFilter::NoPriority => item.priority.is_none(),
        PriorityFilter::Targets(targets) => priority_matches_target(item.priority_char(), targets),
    }
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

fn matches_project_filter(filters: Option<&TextFilterCriteria>, item: &TaskItem) -> bool {
    filters.is_none_or(|filters| {
        filters.filters().iter().all(|filter| match filter {
            TextFilter::Include(target) => task_item_project_matches(item, target),
            TextFilter::Exclude(target) => !task_item_project_matches(item, target),
        })
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

fn parse_priority_filter(value: &str) -> PriorityFilter {
    if value.eq_ignore_ascii_case("none") {
        PriorityFilter::NoPriority
    } else {
        let targets = priority_filter_targets(value);
        if targets.is_empty() {
            PriorityFilter::NoPriority
        } else {
            PriorityFilter::Targets(targets)
        }
    }
}

fn parse_date_filter(value: &str, today: NaiveDate) -> Result<TaskDateFilter> {
    let values: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if values.len() > 1 {
        return values
            .into_iter()
            .map(|value| parse_single_date_filter(value, today))
            .collect::<Result<Vec<_>>>()
            .map(TaskDateFilter::Any);
    }
    parse_single_date_filter(value.trim(), today)
}

fn parse_single_date_filter(value: &str, today: NaiveDate) -> Result<TaskDateFilter> {
    match value.to_ascii_lowercase().as_str() {
        "today" => Ok(TaskDateFilter::Today),
        "week" => Ok(TaskDateFilter::Week),
        "overdue" => Ok(TaskDateFilter::Overdue),
        "upcoming" => Ok(TaskDateFilter::Upcoming),
        _ => Ok(TaskDateFilter::Exact(parse_modifier_style_filter_date(
            value, today,
        )?)),
    }
}

fn parse_modifier_style_filter_date(value: &str, today: NaiveDate) -> Result<NaiveDate> {
    let parsed = parse_task_date_arg_on("task filter", value, today)?;
    let date = parsed
        .as_str()
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid task filter date '{value}'."))?;
    parse_date(date)
}

fn parse_datetime_filter(name: &str, value: &str, today: NaiveDate) -> Result<NaiveDateTime> {
    let parsed = parse_task_date_arg_on(name, value, today)?;
    NaiveDateTime::parse_from_str(parsed.as_str(), "%Y-%m-%d %H:%M")
        .or_else(|_| parse_date(parsed.as_str()).map(|date| date.and_hms_opt(0, 0, 0).unwrap()))
        .map_err(|_| anyhow::anyhow!("Invalid date/time '{value}'."))
}

fn parse_date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Invalid date '{value}'. Use YYYY-MM-DD."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_text_filters(criteria: &Option<TextFilterCriteria>, expected: &[TextFilter]) {
        assert_eq!(
            criteria.as_ref().map(TextFilterCriteria::filters),
            Some(expected)
        );
    }

    fn matches_state_filter_value(
        value: Option<&str>,
        filters: &str,
        open_todo_states: &[String],
        closed_todo_states: &[String],
    ) -> bool {
        let filters = TextFilterCriteria::parse(filters);
        matches_state_filter(value, Some(&filters), open_todo_states, closed_todo_states)
    }

    #[test]
    fn defaults_to_pkms_source() {
        assert_eq!(
            parse_task_filters(&[]).unwrap().source,
            SourceSelection::Pkms
        );
    }

    #[test]
    fn parses_source_aliases() {
        assert_eq!(
            parse_task_filters(&["src:pkms".to_string()])
                .unwrap()
                .source,
            SourceSelection::Pkms
        );
        assert_eq!(
            parse_task_filters(&["source:todoist".to_string()])
                .unwrap()
                .source,
            SourceSelection::Todoist
        );
    }

    #[test]
    fn repeated_sources_combine_to_all() {
        assert_eq!(
            parse_task_filters(&["source:pkms".to_string(), "source:todoist".to_string()])
                .unwrap()
                .source,
            SourceSelection::All
        );
    }

    #[test]
    fn rejects_unknown_filters_initially() {
        assert!(parse_task_filters(&["unknown:value".to_string()]).is_err());
        assert!(parse_task_filters(&["source:remote".to_string()]).is_err());
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
        assert_text_filters(
            &filters.criteria.state,
            &[
                TextFilter::Include("TODO".to_string()),
                TextFilter::Exclude("WAITING".to_string()),
            ],
        );
        assert_text_filters(
            &filters.criteria.tags,
            &[
                TextFilter::Exclude("tag1".to_string()),
                TextFilter::Include("tag2".to_string()),
            ],
        );
        assert_text_filters(
            &filters.criteria.kind,
            &[TextFilter::Include("SCHED".to_string())],
        );
        assert_eq!(filters.criteria.prio, Some(PriorityFilter::NoPriority));
        assert_eq!(filters.criteria.date, Some(TaskDateFilter::Week));
        assert!(filters.criteria.after.is_some());
        assert!(filters.criteria.before.is_some());
        assert_eq!(filters.criteria.scope, vec!["Project Note"]);
        assert_text_filters(
            &filters.criteria.project,
            &[TextFilter::Include("Alpha".to_string())],
        );
    }

    #[test]
    fn parses_comma_separated_priority_filters() {
        let filters = parse_task_filters(&["prio:a,b,c".to_string()]).unwrap();
        assert_eq!(
            filters.criteria.prio,
            Some(PriorityFilter::Targets(vec!['A', 'B', 'C']))
        );
    }

    #[test]
    fn matches_state_meta_filters_against_configured_states() {
        let open_states = vec!["TODO".to_string(), "WAITING".to_string()];
        let closed_states = vec!["DONE".to_string(), "CANCELED".to_string()];

        assert!(matches_state_filter_value(
            Some("TODO"),
            "opened",
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter_value(
            Some("DONE"),
            "closed",
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter_value(
            Some("TODO"),
            "!closed",
            &open_states,
            &closed_states
        ));
        assert!(matches_state_filter_value(
            Some("TODO"),
            "opened,!waiting",
            &open_states,
            &closed_states
        ));
        assert!(!matches_state_filter_value(
            Some("WAITING"),
            "opened,!waiting",
            &open_states,
            &closed_states
        ));
        assert!(!matches_state_filter_value(
            Some("DONE"),
            "opened",
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
    fn parses_modifier_style_date_filter_words() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let filters = parse_task_filters_on(&["date:tom,fri".to_string()], today).unwrap();
        assert_eq!(
            filters.criteria.date,
            Some(TaskDateFilter::Any(vec![
                TaskDateFilter::Exact(NaiveDate::from_ymd_opt(2026, 5, 28).unwrap()),
                TaskDateFilter::Exact(NaiveDate::from_ymd_opt(2026, 5, 29).unwrap()),
            ]))
        );
    }

    #[test]
    fn parses_modifier_style_date_filter_datetime() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let filters = parse_task_filters_on(&["date:2026-05-29 09:30".to_string()], today).unwrap();
        assert_eq!(
            filters.criteria.date,
            Some(TaskDateFilter::Exact(
                NaiveDate::from_ymd_opt(2026, 5, 29).unwrap()
            ))
        );
    }

    #[test]
    fn parses_modifier_style_after_before_filter_words() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let filters =
            parse_task_filters_on(&["after:tom".to_string(), "before:fri".to_string()], today)
                .unwrap();
        assert_eq!(
            filters.criteria.after,
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 28)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        );
        assert_eq!(
            filters.criteria.before,
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 29)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn parses_modifier_style_after_before_filter_datetimes() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let filters = parse_task_filters_on(
            &[
                "after:2026-05-29 09:30".to_string(),
                "before:2026-05-30 18:45".to_string(),
            ],
            today,
        )
        .unwrap();
        assert_eq!(
            filters.criteria.after,
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 29)
                    .unwrap()
                    .and_hms_opt(9, 30, 0)
                    .unwrap()
            )
        );
        assert_eq!(
            filters.criteria.before,
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 30)
                    .unwrap()
                    .and_hms_opt(18, 45, 0)
                    .unwrap()
            )
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
