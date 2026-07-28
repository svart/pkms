use crate::id::TaskId;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use pkms_org::domain::NoteId;
use serde::{Serialize, Serializer};
use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSourceKind {
    Pkms,
}

impl TaskSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskSourceKind::Pkms => "pkms",
        }
    }
}

impl fmt::Display for TaskSourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskSourceKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pkms" => Ok(TaskSourceKind::Pkms),
            _ => anyhow::bail!("Unknown task source '{value}'. Use pkms."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Open,
    Done,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskState(String);

impl TaskState {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TaskState {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for TaskState {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for TaskState {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TaskState {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<TaskState> for String {
    fn from(value: TaskState) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    A,
    B,
    C,
}

impl TaskPriority {
    pub fn as_char(self) -> char {
        match self {
            TaskPriority::A => 'A',
            TaskPriority::B => 'B',
            TaskPriority::C => 'C',
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "A" => Ok(TaskPriority::A),
            "B" => Ok(TaskPriority::B),
            "C" => Ok(TaskPriority::C),
            _ => anyhow::bail!("Invalid priority '{value}'. Use A, B, or C."),
        }
    }

    pub fn from_char(value: char) -> Option<Self> {
        match value.to_ascii_uppercase() {
            'A' => Some(TaskPriority::A),
            'B' => Some(TaskPriority::B),
            'C' => Some(TaskPriority::C),
            _ => None,
        }
    }

    pub fn sort_value(self) -> u8 {
        match self {
            TaskPriority::A => 0,
            TaskPriority::B => 1,
            TaskPriority::C => 2,
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_char().to_string())
    }
}

impl Serialize for TaskPriority {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_char().to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TaskProperty {
    #[serde(rename = "Status")]
    Status,
    #[serde(rename = "Title")]
    Title,
    #[serde(rename = "Priority")]
    Priority,
    #[serde(rename = "Tags")]
    Tags,
    #[serde(rename = "Scheduled")]
    Scheduled,
    #[serde(rename = "Deadline")]
    Deadline,
    #[serde(rename = "Project")]
    Project,
    #[serde(rename = "Description")]
    Description,
    #[serde(rename = "Dependency")]
    Dependency,
}

impl TaskProperty {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskProperty::Status => "Status",
            TaskProperty::Title => "Title",
            TaskProperty::Priority => "Priority",
            TaskProperty::Tags => "Tags",
            TaskProperty::Scheduled => "Scheduled",
            TaskProperty::Deadline => "Deadline",
            TaskProperty::Project => "Project",
            TaskProperty::Description => "Description",
            TaskProperty::Dependency => "Dependency",
        }
    }
}

impl fmt::Display for TaskProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct TaskDateValue(String);

impl TaskDateValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse_naive_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.0, "%Y-%m-%d").ok()
    }
}

impl fmt::Display for TaskDateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TaskDateValue {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for TaskDateValue {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for TaskDateValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TaskDateValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<TaskDateValue> for String {
    fn from(value: TaskDateValue) -> Self {
        value.0
    }
}

impl PartialEq<&str> for TaskDateValue {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for TaskDateValue {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<String> for TaskDateValue {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDate {
    pub raw: String,
    pub date: Option<TaskDateValue>,
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
    pub state: Option<TaskState>,
    pub priority: Option<TaskPriority>,
    pub scheduled: Option<TaskDate>,
    pub deadline: Option<TaskDate>,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub note_title: Option<String>,
    pub note_uuid: Option<NoteId>,
    pub path: Option<PathBuf>,
    pub has_agenda_tag: Option<bool>,
    pub is_daily_file: bool,
    pub daily_file_date: Option<TaskDateValue>,
    pub heading_level: Option<usize>,
    pub line_number: Option<usize>,
    pub url: Option<String>,
    pub is_overdue: bool,
}

impl TaskItem {
    pub fn priority_char(&self) -> Option<char> {
        self.priority.map(TaskPriority::as_char)
    }

    pub fn priority_sort_value(&self) -> u8 {
        self.priority.map(TaskPriority::sort_value).unwrap_or(3)
    }

    pub fn scheduled_date_str(&self) -> Option<&str> {
        self.scheduled
            .as_ref()
            .and_then(|date| date.date.as_ref())
            .map(TaskDateValue::as_str)
    }

    pub fn deadline_date_str(&self) -> Option<&str> {
        self.deadline
            .as_ref()
            .and_then(|date| date.date.as_ref())
            .map(TaskDateValue::as_str)
    }

    pub fn implicit_daily_file_date(&self) -> Option<&str> {
        if self.scheduled.is_none() && self.deadline.is_none() {
            self.daily_file_date.as_ref().map(TaskDateValue::as_str)
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
                pkms_org::org_date::parse_org_date(&date.raw).map(|parsed| {
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
