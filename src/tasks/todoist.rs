use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDate, TaskItem, TaskSourceKind, TaskStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize)]
struct QuickAddRequest<'a> {
    text: &'a str,
    meta: bool,
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

    pub fn quick_add(&self, text: &str) -> Result<serde_json::Value> {
        self.post_json("/tasks/quick", &QuickAddRequest { text, meta: false })
    }

    pub fn close_task(&self, id: &str) -> Result<()> {
        let _: serde_json::Value =
            self.post_json(&format!("/tasks/{id}/close"), &serde_json::json!({}))?;
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
    let id = TaskId::Todoist(task.id);
    let display_id = id.display_id();
    let source_id = id.source_id();
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
        project: task.project_id,
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
