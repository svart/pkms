use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDate, TaskItem, TaskSourceKind, TaskStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://api.todoist.com/api/v1";

pub struct TodoistClient {
    base_url: String,
    token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoistTask {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub due: Option<TodoistDue>,
    #[serde(default)]
    pub deadline: Option<TodoistDeadline>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoistDue {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub string: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoistDeadline {
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoistProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoistLabel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TodoistMetadata {
    projects_by_id: HashMap<String, TodoistProject>,
}

impl TodoistMetadata {
    pub fn new(projects: Vec<TodoistProject>) -> Self {
        let projects_by_id = projects
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect();
        TodoistMetadata { projects_by_id }
    }

    pub fn project_name(&self, id: &str) -> Option<&str> {
        self.projects_by_id
            .get(id)
            .map(|project| project.name.as_str())
    }

    pub fn resolve_project_id(&self, value: &str) -> Result<String> {
        if self.projects_by_id.contains_key(value) {
            return Ok(value.to_string());
        }

        let matches: Vec<&TodoistProject> = self
            .projects_by_id
            .values()
            .filter(|project| project.name.eq_ignore_ascii_case(value))
            .collect();

        match matches.as_slice() {
            [project] => Ok(project.id.clone()),
            [] => anyhow::bail!("Todoist project '{value}' was not found"),
            _ => anyhow::bail!(
                "Todoist project name '{value}' matches multiple projects; use the project id"
            ),
        }
    }
}

#[derive(Debug, Serialize)]
struct QuickAddRequest<'a> {
    text: &'a str,
    meta: bool,
}

#[derive(Debug, Serialize)]
pub struct TodoistCreateTaskRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Paginated<T> {
    results: Vec<T>,
    #[serde(default)]
    next_cursor: Option<String>,
}

impl TodoistClient {
    pub fn new(token: String) -> Self {
        Self::with_base_url(DEFAULT_BASE_URL, token)
    }

    pub fn with_base_url(base_url: impl Into<String>, token: String) -> Self {
        TodoistClient {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
        }
    }

    pub fn list_tasks(&self) -> Result<Vec<TodoistTask>> {
        self.get_paginated("/tasks", &[])
    }

    pub fn filter_tasks(&self, filter: &str) -> Result<Vec<TodoistTask>> {
        self.get_paginated("/tasks/filter", &[("query", filter)])
    }

    pub fn get_task(&self, id: &str) -> Result<TodoistTask> {
        self.get_json(&format!("/tasks/{id}"))
    }

    pub fn list_projects(&self) -> Result<Vec<TodoistProject>> {
        self.get_paginated("/projects", &[])
    }

    pub fn list_labels(&self) -> Result<Vec<TodoistLabel>> {
        self.get_paginated("/labels", &[])
    }

    pub fn quick_add(&self, text: &str) -> Result<serde_json::Value> {
        self.post_json("/tasks/quick", &QuickAddRequest { text, meta: false })
    }

    pub fn create_task(&self, request: &TodoistCreateTaskRequest) -> Result<TodoistTask> {
        self.post_json("/tasks", request)
    }

    pub fn update_task(&self, id: &str, request: &serde_json::Value) -> Result<()> {
        let _: serde_json::Value = self.post_json(&format!("/tasks/{id}"), request)?;
        Ok(())
    }

    pub fn close_task(&self, id: &str) -> Result<()> {
        let _: serde_json::Value =
            self.post_json(&format!("/tasks/{id}/close"), &serde_json::json!({}))?;
        Ok(())
    }

    pub fn reopen_task(&self, id: &str) -> Result<()> {
        let _: serde_json::Value =
            self.post_json(&format!("/tasks/{id}/reopen"), &serde_json::json!({}))?;
        Ok(())
    }

    pub fn delete_task(&self, id: &str) -> Result<()> {
        ureq::delete(&self.url(&format!("/tasks/{id}")))
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(todoist_error)?;
        Ok(())
    }

    fn get_paginated<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut results = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut page_params = params.to_vec();
            page_params.push(("limit", "200"));
            if let Some(ref cursor) = cursor {
                page_params.push(("cursor", cursor.as_str()));
            }
            let page: Paginated<T> = self.get_json(&path_with_query(path, &page_params))?;
            results.extend(page.results);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(results)
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let mut response = ureq::get(&self.url(path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(todoist_error)?;
        response
            .body_mut()
            .read_json()
            .context("Failed to parse Todoist response")
    }

    fn post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let mut response = ureq::post(&self.url(path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .send_json(body)
            .map_err(todoist_error)?;
        response
            .body_mut()
            .read_json()
            .context("Failed to parse Todoist response")
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

pub fn task_to_item(task: TodoistTask) -> TaskItem {
    task_to_item_with_metadata(task, None)
}

pub fn task_to_item_with_metadata(
    task: TodoistTask,
    metadata: Option<&TodoistMetadata>,
) -> TaskItem {
    let id = TaskId::Todoist(task.id);
    let display_id = id.display_id();
    let source_id = id.source_id();
    let project_id = task.project_id;
    let project = project_id.as_ref().map(|id| {
        metadata
            .and_then(|metadata| metadata.project_name(id))
            .unwrap_or(id)
            .to_string()
    });
    TaskItem {
        id,
        display_id,
        source: TaskSourceKind::Todoist,
        source_id,
        title: task.content,
        body: non_empty(task.description),
        status: TaskStatus::Open,
        state: Some("open".to_string()),
        priority: task.priority.and_then(todoist_priority),
        scheduled: task.due.map(|due| TaskDate {
            raw: due.string.or_else(|| due.date.clone()).unwrap_or_default(),
            date: due.date,
        }),
        deadline: task.deadline.map(|deadline| TaskDate {
            raw: deadline.date.clone().unwrap_or_default(),
            date: deadline.date,
        }),
        tags: task.labels,
        project,
        project_id,
        note_title: None,
        note_uuid: None,
        path: None,
        line_number: None,
        url: task.url,
        is_overdue: false,
    }
}

fn todoist_priority(priority: u8) -> Option<String> {
    match priority {
        4 => Some("A".to_string()),
        3 => Some("B".to_string()),
        2 => Some("C".to_string()),
        _ => None,
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn path_with_query(path: &str, params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return path.to_string();
    }
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn todoist_error(err: ureq::Error) -> anyhow::Error {
    match err {
        ureq::Error::StatusCode(code) => anyhow::anyhow!("Todoist API returned HTTP {code}"),
        other => anyhow::anyhow!("Todoist API request failed: {other}"),
    }
}

pub fn ensure_enabled(config: &crate::config::ResolvedConfig) -> Result<String> {
    config.todoist_token()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_filter_query_url() {
        assert_eq!(
            path_with_query(
                "/tasks/filter",
                &[("query", "today | overdue"), ("limit", "200")]
            ),
            "/tasks/filter?query=today%20%7C%20overdue&limit=200"
        );
    }
}
