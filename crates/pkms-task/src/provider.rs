use crate::clock::TaskClock;
use crate::filter::TaskFilters;
use crate::model::{TaskItem, TaskSourceKind};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListView {
    All,
    Agenda,
    Today,
    Week,
    Overdue,
    Upcoming { days: i64 },
    Inbox,
}

#[derive(Debug, Clone)]
pub struct TaskQuery {
    pub filters: TaskFilters,
    pub view: TaskListView,
    pub clock: TaskClock,
}

pub trait TaskProvider {
    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>>;
    fn projects(&self) -> Result<Vec<TaskMetadataRow>>;
    fn tags(&self) -> Result<Vec<TaskMetadataRow>>;
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskMetadataRow {
    pub source: TaskSourceKind,
    pub id: String,
    pub name: String,
    pub count: Option<usize>,
}
