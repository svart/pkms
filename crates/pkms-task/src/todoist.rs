use crate::id::TaskId;
use crate::model::{
    TaskDate, TaskDateValue, TaskItem, TaskPriority, TaskSourceKind, TaskState, TaskStatus,
};
use anyhow::{Context, Result};
use pkms_org::domain::NoteId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const HTTP_LOG_TARGET: &str = "pkms::tasks::todoist::http";
const PKMS_NOTE_MARKER_PREFIX: &str = "pkms:id:";

pub struct TodoistClient {
    base_url: String,
    token: String,
    agent: ureq::Agent,
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
    #[serde(default)]
    pub is_recurring: Option<bool>,
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

pub fn pkms_note_marker_uuid(description: &str) -> Option<NoteId> {
    description.split_whitespace().find_map(|part| {
        part.strip_prefix(PKMS_NOTE_MARKER_PREFIX)
            .filter(|uuid| !uuid.is_empty())
            .map(NoteId::new)
    })
}

pub fn enrich_items_with_pkms_notes(
    config: &pkms_org::OrgConfig,
    items: &mut [TaskItem],
) -> Result<()> {
    if !items.iter().any(|item| {
        item.body
            .as_deref()
            .and_then(pkms_note_marker_uuid)
            .is_some()
    }) {
        return Ok(());
    }

    let graph = pkms_org::Graph::load(config)?;
    for item in items {
        let Some(uuid) = item.body.as_deref().and_then(pkms_note_marker_uuid) else {
            continue;
        };
        item.note_uuid = Some(uuid.clone());
        if let Some(node) = graph.find_node(&uuid) {
            item.note_title = Some(node.title.clone());
        }
    }
    Ok(())
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
    pub fn with_base_url(base_url: impl Into<String>, token: String) -> Self {
        TodoistClient {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
            agent: todoist_agent(),
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
        self.post_no_content(&format!("/tasks/{id}"), request)
    }

    pub fn close_task(&self, id: &str) -> Result<()> {
        self.post_no_content(&format!("/tasks/{id}/close"), &serde_json::json!({}))
    }

    pub fn reopen_task(&self, id: &str) -> Result<()> {
        self.post_no_content(&format!("/tasks/{id}/reopen"), &serde_json::json!({}))
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
            let page_count = page.results.len();
            let total_count = results.len() + page_count;
            let has_next_cursor = page
                .next_cursor
                .as_ref()
                .is_some_and(|next| !next.is_empty());
            tracing::debug!(
                target: HTTP_LOG_TARGET,
                path,
                page_count,
                total_count,
                has_next_cursor,
                "todoist page received"
            );
            results.extend(page.results);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        tracing::debug!(
            target: HTTP_LOG_TARGET,
            path,
            total_count = results.len(),
            "todoist pagination completed"
        );
        Ok(results)
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        log_todoist_request("GET", path);
        let mut response = self
            .agent
            .get(&self.url(path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(|err| {
                log_todoist_error("GET", path, &err);
                todoist_error(err)
            })?;
        log_todoist_response(
            "GET",
            path,
            response.status().as_u16(),
            response_content_length(&response),
        );
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
        log_todoist_request("POST", path);
        let mut response = self
            .agent
            .post(&self.url(path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .send_json(body)
            .map_err(|err| {
                log_todoist_error("POST", path, &err);
                todoist_error(err)
            })?;
        log_todoist_response(
            "POST",
            path,
            response.status().as_u16(),
            response_content_length(&response),
        );
        response
            .body_mut()
            .read_json()
            .context("Failed to parse Todoist response")
    }

    fn post_no_content<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        log_todoist_request("POST", path);
        let response = self
            .agent
            .post(&self.url(path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .send_json(body)
            .map_err(|err| {
                log_todoist_error("POST", path, &err);
                todoist_error(err)
            })?;
        log_todoist_response(
            "POST",
            path,
            response.status().as_u16(),
            response_content_length(&response),
        );
        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn todoist_agent() -> ureq::Agent {
    let tls_config = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls_config)
        .build()
        .new_agent()
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
        state: Some(TaskState::new("open")),
        priority: task.priority.and_then(todoist_priority),
        scheduled: task
            .due
            .map(|due| normalized_task_date(due.date, due.string)),
        deadline: task
            .deadline
            .map(|deadline| normalized_task_date(deadline.date, None)),
        tags: task.labels,
        project,
        project_id,
        note_title: None,
        note_uuid: None,
        path: None,
        has_agenda_tag: None,
        is_daily_file: false,
        daily_file_date: None,
        heading_level: None,
        line_number: None,
        url: task.url,
        is_overdue: false,
    }
}

fn todoist_priority(priority: u8) -> Option<TaskPriority> {
    match priority {
        4 => Some(TaskPriority::A),
        3 => Some(TaskPriority::B),
        2 => Some(TaskPriority::C),
        _ => None,
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn normalized_task_date(date: Option<String>, fallback_raw: Option<String>) -> TaskDate {
    let normalized = date.as_deref().map(normalize_todoist_date);
    let raw = normalized
        .as_deref()
        .map(|date| format!("<{date}>"))
        .or(fallback_raw)
        .unwrap_or_default();
    let date = date
        .as_deref()
        .map(|date| TaskDateValue::new(date.split('T').next().unwrap_or(date)));
    TaskDate { raw, date }
}

fn normalize_todoist_date(date: &str) -> String {
    let Some((day, time)) = date.split_once('T') else {
        return date.to_string();
    };
    let hour_minute = time.get(..5).unwrap_or(time);
    format!("{day} {hour_minute}")
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

fn log_todoist_request(method: &'static str, path: &str) {
    tracing::debug!(
        target: HTTP_LOG_TARGET,
        method,
        path,
        "todoist request"
    );
}

fn log_todoist_response(
    method: &'static str,
    path: &str,
    status: u16,
    content_length: Option<u64>,
) {
    tracing::debug!(
        target: HTTP_LOG_TARGET,
        method,
        path,
        status,
        content_length,
        "todoist response"
    );
}

fn log_todoist_error(method: &'static str, path: &str, err: &ureq::Error) {
    tracing::debug!(
        target: HTTP_LOG_TARGET,
        method,
        path,
        error = %err,
        "todoist request failed"
    );
}

fn response_content_length(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
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
