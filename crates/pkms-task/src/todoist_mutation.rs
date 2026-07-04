use crate::clock::TaskClock;
use crate::config::TodoistProviderConfig;
use crate::model::TaskItem;
use crate::modifiers::TaskModifierSpec;
use crate::mutation::TaskModOutput;
use anyhow::{Result, bail};
use serde::Serialize;

#[cfg(feature = "todoist")]
use crate::model::TaskProperty;

#[derive(Debug, Serialize)]
pub struct TodoistDoneOutput {
    pub source: &'static str,
    pub id: String,
    pub remote_id: String,
    pub completed: bool,
    pub dry_run: bool,
}

pub enum TodoistStateOutput {
    Done(TodoistDoneOutput),
    OpenDryRun { id: String },
    Opened(Box<TaskItem>),
}

#[cfg(feature = "todoist")]
pub fn set_todoist_state(
    config: &TodoistProviderConfig,
    id: &str,
    requested_state: &str,
    dry_run: bool,
) -> Result<TodoistStateOutput> {
    match requested_state.to_ascii_lowercase().as_str() {
        "done" => close_todoist_task(config, id, dry_run).map(TodoistStateOutput::Done),
        "open" => reopen_todoist_task(config, id, dry_run),
        _ => bail!("Todoist state supports only 'open' and 'done'."),
    }
}

#[cfg(not(feature = "todoist"))]
pub fn set_todoist_state(
    _config: &TodoistProviderConfig,
    _id: &str,
    _requested_state: &str,
    _dry_run: bool,
) -> Result<TodoistStateOutput> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub fn add_todoist_task(
    config: &TodoistProviderConfig,
    spec: &TaskModifierSpec,
) -> Result<TaskItem> {
    if spec.state.is_some() {
        bail!("state is available only for PKMS task creation.");
    }
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }
    if is_structured_add(spec) {
        return create_structured_task(config, spec);
    }
    quick_add_task(config, spec)
}

#[cfg(not(feature = "todoist"))]
pub fn add_todoist_task(
    _config: &TodoistProviderConfig,
    spec: &TaskModifierSpec,
) -> Result<TaskItem> {
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub fn mod_todoist_task(
    config: &TodoistProviderConfig,
    id: &str,
    spec: &TaskModifierSpec,
) -> Result<TaskModOutput> {
    use crate::mutation::{mod_date, mod_optional_text, mod_title, validate_mod_source};
    use anyhow::Context;
    use pkms_org::org_date::format_org_date;

    validate_mod_source(spec, "todoist")?;
    if spec.state.is_some() {
        bail!("state is available only for PKMS task modification.");
    }
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }

    let client = crate::todoist::TodoistClient::with_base_url(
        config.api_base_url.clone(),
        config.token.clone(),
    );
    let existing = client.get_task(id)?;
    let needs_metadata = existing.project_id.is_some() || spec.project.is_some();
    let metadata = if needs_metadata {
        Some(crate::todoist::TodoistMetadata::new(
            client.list_projects()?,
        ))
    } else {
        None
    };
    let old_item = crate::todoist::task_to_item_with_metadata(existing.clone(), metadata.as_ref());
    let mut request = serde_json::Map::new();
    let mut changes = Vec::new();

    if let Some(title) = mod_title(spec)? {
        push_change(
            &mut changes,
            TaskProperty::Title,
            Some(old_item.title.clone()),
            Some(title.clone()),
        );
        request.insert("content".to_string(), serde_json::Value::String(title));
    }
    if let Some(description) = spec.description.as_deref() {
        let new = description.trim().to_string();
        push_change(
            &mut changes,
            TaskProperty::Description,
            old_item.body.clone(),
            (!new.is_empty()).then_some(new.clone()),
        );
        request.insert("description".to_string(), serde_json::Value::String(new));
    }
    if let Some(labels) = &spec.labels {
        let old = (!old_item.tags.is_empty()).then(|| old_item.tags.join(", "));
        let new = (!labels.is_empty()).then(|| labels.join(", "));
        push_change(&mut changes, TaskProperty::Tags, old, new);
        request.insert("labels".to_string(), serde_json::json!(labels));
    }
    if let Some(priority) = spec.priority {
        let priority = match priority {
            crate::modifiers::TaskPriorityArg::Clear => 1,
            crate::modifiers::TaskPriorityArg::Set(priority) => priority_value(priority),
        };
        push_change(
            &mut changes,
            TaskProperty::Priority,
            old_item.priority.map(|priority| priority.to_string()),
            priority_label(priority),
        );
        request.insert("priority".to_string(), serde_json::json!(priority));
    }
    if let Some(date) = mod_date(spec.due.as_ref()) {
        if old_item.scheduled_date_str() != date.as_deref() {
            let new_raw = date.as_ref().map(format_org_date).transpose()?;
            push_change(
                &mut changes,
                TaskProperty::Scheduled,
                old_item.scheduled.as_ref().map(|date| date.raw.clone()),
                new_raw,
            );
        }
        request.insert(
            "due_date".to_string(),
            date.map_or(serde_json::Value::Null, |date| {
                serde_json::Value::String(date.to_string())
            }),
        );
    }
    if let Some(date) = mod_date(spec.deadline.as_ref()) {
        if old_item.deadline_date_str() != date.as_deref() {
            let new_raw = date.as_ref().map(format_org_date).transpose()?;
            push_change(
                &mut changes,
                TaskProperty::Deadline,
                old_item.deadline.as_ref().map(|date| date.raw.clone()),
                new_raw,
            );
        }
        request.insert(
            "deadline_date".to_string(),
            date.map_or(serde_json::Value::Null, |date| {
                serde_json::Value::String(date.to_string())
            }),
        );
    }
    if let Some(project) = mod_optional_text(spec.project.as_deref()) {
        let (project_id, project_name) = match project {
            Some(project) => {
                let metadata = metadata
                    .as_ref()
                    .context("Todoist project metadata was not loaded")?;
                let project_id = metadata.resolve_project_id(&project)?;
                let project_name = metadata
                    .project_name(&project_id)
                    .unwrap_or(&project_id)
                    .to_string();
                (serde_json::Value::String(project_id), Some(project_name))
            }
            None => (serde_json::Value::Null, None),
        };
        push_change(
            &mut changes,
            TaskProperty::Project,
            old_item.project.clone(),
            project_name,
        );
        request.insert("project_id".to_string(), project_id);
    }

    changes.retain(|change| change.old != change.new);
    if changes.is_empty() {
        return Ok(TaskModOutput {
            changed: false,
            id: format!("todoist:{id}"),
            changes,
            item: None,
        });
    }

    client.update_task(id, &serde_json::Value::Object(request))?;
    let task = client.get_task(id)?;
    let metadata = metadata_for_task(&client, &task)?.or(metadata);
    let mut item = crate::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::todoist::enrich_items_with_pkms_notes(&config.org, std::slice::from_mut(&mut item))?;
    Ok(TaskModOutput {
        changed: true,
        id: format!("todoist:{id}"),
        changes,
        item: Some(item),
    })
}

#[cfg(not(feature = "todoist"))]
pub fn mod_todoist_task(
    _config: &TodoistProviderConfig,
    _id: &str,
    _spec: &TaskModifierSpec,
) -> Result<TaskModOutput> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub fn postpone_todoist_task(
    config: &TodoistProviderConfig,
    id: &str,
    to: &str,
    clock: TaskClock,
) -> Result<TaskItem> {
    let client = crate::todoist::TodoistClient::with_base_url(
        config.api_base_url.clone(),
        config.token.clone(),
    );
    let existing = client.get_task(id)?;
    let is_recurring = existing
        .due
        .as_ref()
        .and_then(|due| due.is_recurring)
        .unwrap_or(false);
    if !is_recurring {
        bail!("Todoist task todoist:{id} is not recurring");
    }
    client.update_task(
        id,
        &serde_json::json!({ "due_date": crate::mutation::parse_mutation_due_date(to, clock.today)? }),
    )?;
    let task = client.get_task(id)?;
    let metadata = metadata_for_task(&client, &task)?;
    let mut item = crate::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::todoist::enrich_items_with_pkms_notes(&config.org, std::slice::from_mut(&mut item))?;
    Ok(item)
}

#[cfg(not(feature = "todoist"))]
pub fn postpone_todoist_task(
    _config: &TodoistProviderConfig,
    _id: &str,
    _to: &str,
    _clock: TaskClock,
) -> Result<TaskItem> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub fn close_todoist_task(
    config: &TodoistProviderConfig,
    id: &str,
    dry_run: bool,
) -> Result<TodoistDoneOutput> {
    if !dry_run {
        let client = crate::todoist::TodoistClient::with_base_url(
            config.api_base_url.clone(),
            config.token.clone(),
        );
        client.close_task(id)?;
    }

    Ok(TodoistDoneOutput {
        source: "todoist",
        id: format!("todoist:{id}"),
        remote_id: id.to_string(),
        completed: !dry_run,
        dry_run,
    })
}

#[cfg(not(feature = "todoist"))]
pub fn close_todoist_task(
    _config: &TodoistProviderConfig,
    _id: &str,
    _dry_run: bool,
) -> Result<TodoistDoneOutput> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn reopen_todoist_task(
    config: &TodoistProviderConfig,
    id: &str,
    dry_run: bool,
) -> Result<TodoistStateOutput> {
    if dry_run {
        return Ok(TodoistStateOutput::OpenDryRun {
            id: format!("todoist:{id}"),
        });
    }
    let client = crate::todoist::TodoistClient::with_base_url(
        config.api_base_url.clone(),
        config.token.clone(),
    );
    client.reopen_task(id)?;
    let task = client.get_task(id)?;
    let metadata = metadata_for_task(&client, &task)?;
    let mut item = crate::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::todoist::enrich_items_with_pkms_notes(&config.org, std::slice::from_mut(&mut item))?;
    Ok(TodoistStateOutput::Opened(Box::new(item)))
}

#[cfg(feature = "todoist")]
fn is_structured_add(spec: &TaskModifierSpec) -> bool {
    spec.title.is_some()
        || spec.due.is_some()
        || spec.deadline.is_some()
        || !spec.labels().is_empty()
        || spec.priority.is_some()
        || spec.description.is_some()
}

#[cfg(feature = "todoist")]
fn create_structured_task(
    config: &TodoistProviderConfig,
    spec: &TaskModifierSpec,
) -> Result<TaskItem> {
    let client = crate::todoist::TodoistClient::with_base_url(
        config.api_base_url.clone(),
        config.token.clone(),
    );
    if spec.title.is_some() && spec.text.is_some() {
        bail!("Structured Todoist task creation uses title: or positional text, not both.");
    }
    let title = spec
        .title
        .as_deref()
        .or(spec.text.as_deref())
        .filter(|title| !title.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Structured Todoist task creation requires title:"))?;
    let description = spec.description.clone();
    let due_date = add_date("due", spec.due.as_ref())?;
    let deadline_date = add_date("deadline", spec.deadline.as_ref())?;
    let priority = spec.priority.map(add_priority).transpose()?;
    let metadata = crate::todoist::TodoistMetadata::new(client.list_projects()?);
    let project_id = spec
        .project
        .as_deref()
        .map(|project| metadata.resolve_project_id(project))
        .transpose()?;
    let request = crate::todoist::TodoistCreateTaskRequest {
        content: title.to_string(),
        description,
        project_id,
        labels: spec.labels().to_vec(),
        priority,
        due_date,
        deadline_date,
    };

    let task = client.create_task(&request)?;
    let mut item = crate::todoist::task_to_item_with_metadata(task, Some(&metadata));
    crate::todoist::enrich_items_with_pkms_notes(&config.org, std::slice::from_mut(&mut item))?;
    Ok(item)
}

#[cfg(feature = "todoist")]
fn quick_add_task(config: &TodoistProviderConfig, spec: &TaskModifierSpec) -> Result<TaskItem> {
    let client = crate::todoist::TodoistClient::with_base_url(
        config.api_base_url.clone(),
        config.token.clone(),
    );
    let text = spec
        .text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Todoist Quick Add requires task text or title:"))?;
    let text = quick_add_text(text, spec.project.as_deref());
    let response = client.quick_add(&text)?;
    let id = created_task_id(&response)?;
    let task = client.get_task(&id)?;
    let metadata = crate::todoist::TodoistMetadata::new(client.list_projects()?);
    let mut item = crate::todoist::task_to_item_with_metadata(task, Some(&metadata));
    crate::todoist::enrich_items_with_pkms_notes(&config.org, std::slice::from_mut(&mut item))?;
    Ok(item)
}

#[cfg(feature = "todoist")]
fn push_change(
    changes: &mut Vec<crate::mutation::TaskModChange>,
    property: TaskProperty,
    old: Option<String>,
    new: Option<String>,
) {
    changes.push(crate::mutation::TaskModChange { property, old, new });
}

#[cfg(feature = "todoist")]
fn priority_label(priority: u8) -> Option<String> {
    match priority {
        4 => Some("A".to_string()),
        3 => Some("B".to_string()),
        2 => Some("C".to_string()),
        _ => None,
    }
}

#[cfg(feature = "todoist")]
fn metadata_for_task(
    client: &crate::todoist::TodoistClient,
    task: &crate::todoist::TodoistTask,
) -> Result<Option<crate::todoist::TodoistMetadata>> {
    if task.project_id.is_some() {
        Ok(Some(crate::todoist::TodoistMetadata::new(
            client.list_projects()?,
        )))
    } else {
        Ok(None)
    }
}

#[cfg(feature = "todoist")]
fn created_task_id(response: &serde_json::Value) -> Result<String> {
    response
        .get("id")
        .or_else(|| response.get("task_id"))
        .or_else(|| response.get("task").and_then(|task| task.get("id")))
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Todoist create response did not include a task id"))
}

#[cfg(feature = "todoist")]
fn add_date(name: &str, value: Option<&crate::modifiers::TaskDateArg>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(crate::modifiers::TaskDateArg::Set(date)) => Ok(Some(date.to_string())),
        Some(crate::modifiers::TaskDateArg::Clear) => {
            bail!("{name} cannot be cleared when creating a task.")
        }
    }
}

#[cfg(feature = "todoist")]
fn add_priority(priority: crate::modifiers::TaskPriorityArg) -> Result<u8> {
    match priority {
        crate::modifiers::TaskPriorityArg::Set(priority) => Ok(priority_value(priority)),
        crate::modifiers::TaskPriorityArg::Clear => {
            bail!("Invalid priority 'none'. Use A, B, or C.")
        }
    }
}

#[cfg(feature = "todoist")]
fn priority_value(priority: crate::model::TaskPriority) -> u8 {
    match priority {
        crate::model::TaskPriority::A => 4,
        crate::model::TaskPriority::B => 3,
        crate::model::TaskPriority::C => 2,
    }
}

#[cfg(feature = "todoist")]
fn quick_add_text(text: &str, project: Option<&str>) -> String {
    match project {
        Some(project) if !project.is_empty() => {
            format!("{text} #{}", project.replace(' ', "\\ "))
        }
        _ => text.to_string(),
    }
}
