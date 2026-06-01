use crate::tasks::id::TaskId;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSourceKind {
    Pkms,
    Todoist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Open,
    Done,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDate {
    pub raw: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskItem {
    pub id: TaskId,
    pub display_id: String,
    pub source: TaskSourceKind,
    pub source_id: String,
    pub title: String,
    pub body: Option<String>,
    pub status: TaskStatus,
    pub state: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<TaskDate>,
    pub deadline: Option<TaskDate>,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub note_title: Option<String>,
    pub note_uuid: Option<String>,
    pub path: Option<PathBuf>,
    pub has_agenda_tag: Option<bool>,
    pub is_daily_file: bool,
    pub daily_file_date: Option<String>,
    pub heading_level: Option<usize>,
    pub line_number: Option<usize>,
    pub url: Option<String>,
    pub is_overdue: bool,
}

impl TaskItem {
    pub fn priority_char(&self) -> Option<char> {
        self.priority
            .as_deref()
            .and_then(|priority| priority.chars().next())
    }

    pub fn priority_sort_value(&self) -> u8 {
        self.priority_char()
            .map(crate::util::priority_value)
            .unwrap_or(3)
    }

    pub fn scheduled_date_str(&self) -> Option<&str> {
        self.scheduled
            .as_ref()
            .and_then(|date| date.date.as_deref())
    }

    pub fn deadline_date_str(&self) -> Option<&str> {
        self.deadline.as_ref().and_then(|date| date.date.as_deref())
    }

    pub fn implicit_daily_file_date(&self) -> Option<&str> {
        if self.scheduled.is_none() && self.deadline.is_none() {
            self.daily_file_date.as_deref()
        } else {
            None
        }
    }

    pub fn effective_date(&self) -> Option<&str> {
        self.scheduled_date_str()
            .or_else(|| self.deadline_date_str())
            .or_else(|| self.implicit_daily_file_date())
    }

    pub fn datetimes(&self) -> Vec<NaiveDateTime> {
        [self.scheduled.as_ref(), self.deadline.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|date| {
                crate::org_date::parse_org_date(&date.raw).map(|parsed| {
                    let time = parsed
                        .time
                        .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                    parsed.base_date.and_time(time)
                })
            })
            .collect()
    }

    pub fn dates(&self) -> Vec<NaiveDate> {
        let mut dates: Vec<NaiveDate> = self.datetimes().into_iter().map(|dt| dt.date()).collect();
        if let Some(daily_file_date) = self.implicit_daily_file_date()
            && let Ok(date) = NaiveDate::parse_from_str(daily_file_date, "%Y-%m-%d")
        {
            dates.push(date);
        }
        dates
    }

    pub fn is_overdue_on(&self, today: NaiveDate) -> bool {
        self.is_overdue || self.dates().into_iter().any(|date| date < today)
    }

    pub fn is_today_on(&self, today: NaiveDate) -> bool {
        self.dates().contains(&today)
    }
}
