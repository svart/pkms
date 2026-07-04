use anyhow::{Result, bail};
use chrono::NaiveDate;

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
