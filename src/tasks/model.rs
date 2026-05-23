use crate::tasks::id::TaskId;
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
    pub note_title: Option<String>,
    pub note_uuid: Option<String>,
    pub path: Option<PathBuf>,
    pub line_number: Option<usize>,
    pub url: Option<String>,
    pub is_overdue: bool,
}
