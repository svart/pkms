use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use crate::tasks::clock::TaskClock;
use crate::tasks::modifiers::TaskModifierSpec;
use anyhow::{Result, bail};
use std::process::ExitCode;

#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
#[cfg(feature = "todoist")]
use crate::tasks::model::{TaskPriority, TaskProperty};
#[cfg(feature = "todoist")]
use crate::tasks::modifiers::{TaskPriorityArg, org_date, parse_task_date_arg_on};
#[cfg(feature = "todoist")]
use anyhow::Context;
#[cfg(feature = "todoist")]
use serde::Serialize;

#[cfg(feature = "todoist")]
use super::super::render;
#[cfg(feature = "todoist")]
use super::{mod_date, mod_optional_text, mod_title, parse_mutation_due_date, validate_mod_source};

#[cfg(feature = "todoist")]
pub(super) fn set_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    match requested_state.to_ascii_lowercase().as_str() {
        "done" => close(config, ctx, id, dry_run),
        "open" => {
            if dry_run {
                println!("Would reopen Todoist task todoist:{id}");
                return Ok(());
            }
            let token = crate::tasks::todoist::ensure_enabled(config)?;
            let client = crate::tasks::todoist::TodoistClient::with_base_url(
                config.todoist_api_base_url(),
                token,
            );
            client.reopen_task(id)?;
            let task = client.get_task(id)?;
            let metadata = metadata_for_task(&client, &task)?;
            let mut item =
                crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
            crate::tasks::todoist::enrich_items_with_pkms_notes(
                config,
                std::slice::from_mut(&mut item),
            )?;
            render::print_mutation_output(ctx, "state-open", item)
        }
        _ => bail!("Todoist state supports only 'open' and 'done'."),
    }
}

#[cfg(not(feature = "todoist"))]
pub(super) fn set_state(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _requested_state: &str,
    _dry_run: bool,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub(super) fn add(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<()> {
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
        return create_structured_task(config, ctx, spec, clock);
    }
    quick_add_task(config, ctx, spec)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn add(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    spec: &TaskModifierSpec,
    _clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<()> {
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
    let due_date = validate_date_arg("due", spec.due.as_deref(), clock.today)?;
    let deadline_date = validate_date_arg("deadline", spec.deadline.as_deref(), clock.today)?;
    let priority = spec.priority.map(add_priority).transpose()?;
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let metadata = crate::tasks::todoist::TodoistMetadata::new(client.list_projects()?);
    let project_id = spec
        .project
        .as_deref()
        .map(|project| metadata.resolve_project_id(project))
        .transpose()?;
    let request = crate::tasks::todoist::TodoistCreateTaskRequest {
        content: title.to_string(),
        description,
        project_id,
        labels: spec.labels().to_vec(),
        priority,
        due_date,
        deadline_date,
    };

    let task = client.create_task(&request)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, Some(&metadata));
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
fn quick_add_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
) -> Result<()> {
    let text = spec
        .text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Todoist Quick Add requires task text or title:"))?;
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let text = quick_add_text(text, spec.project.as_deref());
    let response = client.quick_add(&text)?;
    let id = created_task_id(&response)?;
    let task = client.get_task(&id)?;
    let metadata = crate::tasks::todoist::TodoistMetadata::new(client.list_projects()?);
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, Some(&metadata));
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
pub(super) fn mod_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
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

    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let existing = client.get_task(id)?;
    let needs_metadata = existing.project_id.is_some() || spec.project.is_some();
    let metadata = if needs_metadata {
        Some(crate::tasks::todoist::TodoistMetadata::new(
            client.list_projects()?,
        ))
    } else {
        None
    };
    let old_item =
        crate::tasks::todoist::task_to_item_with_metadata(existing.clone(), metadata.as_ref());
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
            TaskPriorityArg::Clear => 1,
            TaskPriorityArg::Set(priority) => priority_value(priority),
        };
        push_change(
            &mut changes,
            TaskProperty::Priority,
            old_item.priority.map(|priority| priority.to_string()),
            priority_label(priority),
        );
        request.insert("priority".to_string(), serde_json::json!(priority));
    }
    if let Some(date) = mod_date("schedule", spec.due.as_deref(), clock.today)? {
        if old_item.scheduled_date_str() != date.as_deref() {
            let new_raw = date.as_ref().map(org_date).transpose()?;
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
    if let Some(date) = mod_date("deadline", spec.deadline.as_deref(), clock.today)? {
        if old_item.deadline_date_str() != date.as_deref() {
            let new_raw = date.as_ref().map(org_date).transpose()?;
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
        return render::print_mod_output(
            ctx,
            render::TaskModOutput {
                changed: false,
                id: format!("todoist:{id}"),
                changes,
                item: None,
            },
            clock.today,
        );
    }

    client.update_task(id, &serde_json::Value::Object(request))?;
    let task = client.get_task(id)?;
    let metadata = metadata_for_task(&client, &task)?.or(metadata);
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_mod_output(
        ctx,
        render::TaskModOutput {
            changed: true,
            id: format!("todoist:{id}"),
            changes,
            item: Some(item),
        },
        clock.today,
    )
}

#[cfg(not(feature = "todoist"))]
pub(super) fn mod_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _spec: &TaskModifierSpec,
    _clock: TaskClock,
) -> Result<ExitCode> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn push_change(
    changes: &mut Vec<render::TaskModChange>,
    property: TaskProperty,
    old: Option<String>,
    new: Option<String>,
) {
    changes.push(render::TaskModChange { property, old, new });
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
pub(super) fn postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    to: &str,
    clock: TaskClock,
) -> Result<()> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
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
        &serde_json::json!({ "due_date": parse_mutation_due_date(to, clock.today)? }),
    )?;
    let task = client.get_task(id)?;
    let metadata = metadata_for_task(&client, &task)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn postpone(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _to: &str,
    _clock: TaskClock,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn metadata_for_task(
    client: &crate::tasks::todoist::TodoistClient,
    task: &crate::tasks::todoist::TodoistTask,
) -> Result<Option<crate::tasks::todoist::TodoistMetadata>> {
    if task.project_id.is_some() {
        Ok(Some(crate::tasks::todoist::TodoistMetadata::new(
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
fn validate_date_arg(
    name: &str,
    value: Option<&str>,
    today: chrono::NaiveDate,
) -> Result<Option<String>> {
    value
        .map(|value| parse_task_date_arg_on(name, value, today))
        .transpose()
        .map(|date| date.map(|date| date.to_string()))
}

#[cfg(feature = "todoist")]
fn add_priority(priority: TaskPriorityArg) -> Result<u8> {
    match priority {
        TaskPriorityArg::Set(priority) => Ok(priority_value(priority)),
        TaskPriorityArg::Clear => bail!("Invalid priority 'none'. Use A, B, or C."),
    }
}

#[cfg(feature = "todoist")]
fn priority_value(priority: TaskPriority) -> u8 {
    match priority {
        TaskPriority::A => 4,
        TaskPriority::B => 3,
        TaskPriority::C => 2,
    }
}

#[cfg(feature = "todoist")]
pub(super) fn close(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    dry_run: bool,
) -> Result<()> {
    #[derive(Serialize)]
    struct DoneOutput<'a> {
        source: &'static str,
        id: String,
        remote_id: &'a str,
        completed: bool,
        dry_run: bool,
    }

    if !dry_run {
        let token = crate::tasks::todoist::ensure_enabled(config)?;
        let client = crate::tasks::todoist::TodoistClient::with_base_url(
            config.todoist_api_base_url(),
            token,
        );
        client.close_task(id)?;
    }

    let output = DoneOutput {
        source: "todoist",
        id: format!("todoist:{id}"),
        remote_id: id,
        completed: !dry_run,
        dry_run,
    };
    match ctx.format {
        OutputFormat::Text => {
            if dry_run {
                println!("Would complete Todoist task todoist:{id}");
            } else {
                println!("Completed Todoist task todoist:{id}");
            }
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(&output),
        OutputFormat::Ndjson => ctx.print_ndjson(&[output]),
    }
}

#[cfg(not(feature = "todoist"))]
pub(super) fn close(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _dry_run: bool,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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
