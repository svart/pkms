use anyhow::{Result, bail};
use chrono::NaiveDate;
use std::collections::BTreeMap;

use crate::model::TaskItem;

pub const TASK_SORT_FIELD_HELP: &str =
    "priority, date, scheduled, deadline, file, source, state, task, title, or project";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskSortField {
    Priority,
    Date,
    Scheduled,
    Deadline,
    File,
    Source,
    State,
    Task,
    Title,
    Project,
}

impl TaskSortField {
    pub fn parse(field: &str) -> Result<Self> {
        match field {
            "priority" => Ok(TaskSortField::Priority),
            "date" => Ok(TaskSortField::Date),
            "scheduled" => Ok(TaskSortField::Scheduled),
            "deadline" => Ok(TaskSortField::Deadline),
            "file" => Ok(TaskSortField::File),
            "source" => Ok(TaskSortField::Source),
            "state" => Ok(TaskSortField::State),
            "task" => Ok(TaskSortField::Task),
            "title" => Ok(TaskSortField::Title),
            "project" => Ok(TaskSortField::Project),
            other => bail!("Unknown task sort field '{other}'. Use {TASK_SORT_FIELD_HELP}."),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskGroupField {
    State,
    File,
    Priority,
}

impl TaskGroupField {
    pub fn parse(field: &str) -> Result<Self> {
        match field {
            "state" => Ok(TaskGroupField::State),
            "file" => Ok(TaskGroupField::File),
            "priority" => Ok(TaskGroupField::Priority),
            other => bail!("Unknown task group field '{other}'. Use state, file, or priority."),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaskGroupField::State => "state",
            TaskGroupField::File => "file",
            TaskGroupField::Priority => "priority",
        }
    }
}

pub fn apply_limit<T>(items: &mut Vec<T>, limit: Option<usize>) -> usize {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    total
}

pub fn agenda_window_cutoff(today: NaiveDate, days: i64) -> Option<NaiveDate> {
    if days <= 0 {
        return None;
    }
    chrono::Duration::try_days(days - 1).and_then(|duration| today.checked_add_signed(duration))
}

pub fn date_in_agenda_window(date: NaiveDate, today: NaiveDate, days: i64) -> bool {
    agenda_window_cutoff(today, days).is_some_and(|cutoff| date >= today && date <= cutoff)
}

pub fn agenda_day_section_label(today: NaiveDate, date: NaiveDate) -> String {
    match (date - today).num_days() {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ => date.format("%Y-%m-%d %a").to_string(),
    }
}

pub fn parse_task_sort_fields(sort: &str) -> Result<Vec<TaskSortField>> {
    let fields: Vec<&str> = sort
        .split(',')
        .map(|field| field.trim())
        .filter(|field| !field.is_empty())
        .collect();
    if fields.is_empty() {
        bail!("Task sort must include at least one field");
    }
    fields.into_iter().map(TaskSortField::parse).collect()
}

pub fn parse_task_group_field(group_field: &str) -> Result<TaskGroupField> {
    TaskGroupField::parse(group_field)
}

pub fn group_task_items(
    items: Vec<TaskItem>,
    group_field: &str,
    sort: &str,
    limit: Option<usize>,
) -> Result<(TaskGroupField, BTreeMap<String, Vec<TaskItem>>, usize)> {
    let group_field = parse_task_group_field(group_field)?;
    let mut groups: BTreeMap<String, Vec<TaskItem>> = BTreeMap::new();
    for item in items {
        groups
            .entry(task_group_key(&item, group_field))
            .or_default()
            .push(item);
    }

    let total = groups.values().map(Vec::len).sum();
    let sort_fields = if groups.is_empty() {
        Vec::new()
    } else {
        parse_task_sort_fields(sort)?
    };
    for group_items in groups.values_mut() {
        sort_task_items(group_items, &sort_fields);
        if let Some(limit) = limit {
            group_items.truncate(limit);
        }
    }
    Ok((group_field, groups, total))
}

fn task_group_key(item: &TaskItem, group_field: TaskGroupField) -> String {
    match group_field {
        TaskGroupField::State => item.state.as_deref().unwrap_or("NONE").to_string(),
        TaskGroupField::File => item.note_title.clone().unwrap_or_default(),
        TaskGroupField::Priority => match item.priority_char() {
            Some('A') => "Priority A".to_string(),
            Some('B') => "Priority B".to_string(),
            Some('C') => "Priority C".to_string(),
            _ => "No Priority".to_string(),
        },
    }
}

pub fn retain_upcoming_task_items_on(items: &mut Vec<TaskItem>, days: i64, today: NaiveDate) {
    let cutoff = today + chrono::Duration::days(days);
    items.retain(|item| {
        item.effective_date()
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .is_some_and(|date| date > today && date <= cutoff)
    });
}

pub fn retain_agenda_window_task_items_on(items: &mut Vec<TaskItem>, days: i64, today: NaiveDate) {
    items.retain(|item| {
        item.is_overdue_on(today)
            || item
                .dates()
                .into_iter()
                .any(|date| date_in_agenda_window(date, today, days))
    });
}

pub fn sort_task_items(items: &mut [TaskItem], fields: &[TaskSortField]) {
    items.sort_by(|a, b| {
        for &field in fields {
            let ord = match field {
                TaskSortField::Priority => a.priority_sort_value().cmp(&b.priority_sort_value()),
                TaskSortField::Date => a.effective_date().cmp(&b.effective_date()),
                TaskSortField::Scheduled => a.scheduled_date_str().cmp(&b.scheduled_date_str()),
                TaskSortField::Deadline => a.deadline_date_str().cmp(&b.deadline_date_str()),
                TaskSortField::File => a.note_title.cmp(&b.note_title),
                TaskSortField::Source => a.source.as_str().cmp(b.source.as_str()),
                TaskSortField::State => a.state.cmp(&b.state),
                TaskSortField::Task | TaskSortField::Title => a.title.cmp(&b.title),
                TaskSortField::Project => a.project.cmp(&b.project),
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        a.source
            .as_str()
            .cmp(b.source.as_str())
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.title.cmp(&b.title))
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgendaWindow {
    Sections,
    Days(i64),
}

impl AgendaWindow {
    pub fn from_days(days: Option<i64>) -> Self {
        days.map(|days| AgendaWindow::Days(days.max(0)))
            .unwrap_or(AgendaWindow::Sections)
    }

    pub fn days(self) -> Option<i64> {
        match self {
            AgendaWindow::Sections => None,
            AgendaWindow::Days(days) => Some(days),
        }
    }
}
