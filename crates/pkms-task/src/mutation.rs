use crate::clock::TaskClock;
use crate::config::PkmsTaskConfig;
use crate::id::TaskId;
use crate::model::{TaskDateValue, TaskItem};
use crate::modifiers::{
    TaskDateArg, TaskDependencyArg, TaskModifierSpec, TaskPriorityArg, is_clear_value, org_date,
    parse_task_date_arg_on,
};
use crate::pkms::{self, PkmsInboxTarget};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use pkms_org::Graph;
use pkms_org::graph::tasks::TaskLocation as GraphTaskLocation;
use pkms_org::org_task_mutation;
use pkms_org::parser::OrgTodoState;
use serde::Serialize;
use std::path::Path;

pub fn validate_mod_source(spec: &TaskModifierSpec, expected: &str) -> Result<()> {
    if let Some(source) = spec.source
        && !source.as_str().eq_ignore_ascii_case(expected)
    {
        bail!("Task source cannot be changed by task mod.");
    }
    Ok(())
}

pub fn mod_title(spec: &TaskModifierSpec) -> Result<Option<String>> {
    Ok(spec
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string))
}

pub fn mod_optional_text(value: Option<&str>) -> Option<Option<String>> {
    value.map(|value| {
        let value = value.trim();
        (!is_clear_value(value)).then(|| value.to_string())
    })
}

pub fn mod_date(value: Option<&TaskDateArg>) -> Option<Option<TaskDateValue>> {
    value.map(|date| date.as_value().cloned())
}

pub fn parse_mutation_due_date(value: &str, today: NaiveDate) -> Result<String> {
    Ok(parse_task_date_arg_on("due", value, today)?.to_string())
}

pub fn unsupported_task_source(source: impl AsRef<str>) -> Result<()> {
    bail!(
        "Task source '{}' is not configured in this build.",
        source.as_ref()
    )
}

#[derive(Debug, Serialize)]
pub struct TaskStateChangeOutput {
    pub id: String,
    #[serde(skip_serializing)]
    pub title: String,
    pub path: String,
    pub line_number: usize,
    pub old_state: String,
    pub new_state: String,
    pub dry_run: bool,
}

pub fn set_pkms_state(
    config: &PkmsTaskConfig,
    canonical_id: usize,
    requested_state: &str,
    dry_run: bool,
) -> Result<TaskStateChangeOutput> {
    let new_state = canonical_state(config, requested_state)?;
    let graph = Graph::load(&config.org)?;
    let location = graph.resolve_canonical_task_id(&config.task_states, canonical_id)?;
    let title =
        task_title_in_graph(&graph, &location.path, location.line_number).with_context(|| {
            format!(
                "Resolved task but could not find title at {}:{}",
                location.path, location.line_number
            )
        })?;
    let output = org_task_mutation::replace_heading_state(
        &location.path,
        location.line_number,
        &new_state,
        dry_run,
    )?;
    Ok(TaskStateChangeOutput {
        id: TaskId::Pkms(canonical_id).display_id(),
        title,
        path: output.path,
        line_number: output.line_number,
        old_state: output.old_state,
        new_state: output.new_state,
        dry_run,
    })
}

pub fn add_pkms_task(
    config: &PkmsTaskConfig,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<TaskItem> {
    if spec.dependency.is_some() && spec.note.is_some() {
        bail!("note and dep cannot be used together for PKMS task creation.");
    }

    let location = match spec.dependency.as_ref() {
        Some(TaskDependencyArg::Set(TaskId::Pkms(parent_id))) => {
            add_dependency_task(config, spec, *parent_id)?
        }
        Some(TaskDependencyArg::Set(_)) => bail!("dep is available only for PKMS task IDs."),
        Some(TaskDependencyArg::Clear) => bail!("dep requires a PKMS task ID for task creation."),
        None => add_inbox_task(config, spec, clock)?,
    };
    pkms::find_task_item_on(config, &location.path, location.line_number, clock)?.with_context(
        || {
            format!(
                "Created task but could not reload it from {}",
                location.path.display()
            )
        },
    )
}

pub fn postpone_pkms_task(
    config: &PkmsTaskConfig,
    canonical_id: usize,
    to: &str,
    clock: TaskClock,
) -> Result<TaskItem> {
    let date = parse_mutation_due_date(to, clock.today)?;
    let graph = Graph::load(&config.org)?;
    let location = graph.resolve_canonical_task_id(&config.task_states, canonical_id)?;
    org_task_mutation::update_recurring_planning_date(&location.path, location.line_number, &date)?;
    pkms::find_task_item_on(
        config,
        Path::new(&location.path),
        location.line_number,
        clock,
    )?
    .with_context(|| {
        format!(
            "Changed task but could not reload it from {}:{}",
            location.path, location.line_number
        )
    })
}

fn add_inbox_task(
    config: &PkmsTaskConfig,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<pkms::TaskLocation> {
    let inbox_target = match spec.note.as_deref() {
        Some(note) => pkms::resolve_note_task_target(config, note)?,
        None => pkms::resolve_inbox_target_on(config, true, clock.today)?,
    };
    let heading_level = match inbox_target {
        PkmsInboxTarget::Note(_) => 1,
        PkmsInboxTarget::Daily { .. } => 2,
    };
    let entry = format_task_entry(config, spec, heading_level)?;
    pkms::append_inbox_entry(&inbox_target, &entry)
}

fn add_dependency_task(
    config: &PkmsTaskConfig,
    spec: &TaskModifierSpec,
    canonical_id: usize,
) -> Result<pkms::TaskLocation> {
    let graph = Graph::load(&config.org)?;
    let location = graph.resolve_canonical_task_id(&config.task_states, canonical_id)?;
    let location = pkms_task_location(location);
    let parent_level = pkms::heading_level_at(&location)?;
    let entry = format_task_entry(config, spec, parent_level + 1)?;
    pkms::append_child_entry(&location, &entry)
}

fn format_task_entry(
    config: &PkmsTaskConfig,
    spec: &TaskModifierSpec,
    heading_level: usize,
) -> Result<String> {
    let title = add_title(spec)?;
    let state = match spec.state.as_deref() {
        Some(state) => canonical_state(config, state)?.to_string(),
        None => config
            .task_states
            .open_states
            .first()
            .cloned()
            .unwrap_or_else(|| "TODO".to_string()),
    };
    let priority = add_priority(spec)?;
    let labels = spec.labels();
    let tags = if labels.is_empty() {
        String::new()
    } else {
        format!(" :{}:", labels.join(":"))
    };

    let level = "*".repeat(heading_level);
    let mut entry = format!("{level} {state}{priority} {title}{tags}\n");
    let due = add_date("due", spec.due.as_ref())?;
    let deadline = add_date("deadline", spec.deadline.as_ref())?;
    if due.is_some() || deadline.is_some() {
        let mut planning = Vec::new();
        if let Some(due) = due {
            planning.push(format!("SCHEDULED: {}", org_date(due)?));
        }
        if let Some(deadline) = deadline {
            planning.push(format!("DEADLINE: {}", org_date(deadline)?));
        }
        entry.push_str(&format!("{}\n", planning.join(" ")));
    }
    if let Some(description) = spec
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        entry.push('\n');
        entry.push_str(description);
        entry.push('\n');
    }

    Ok(entry)
}

fn add_date<'a>(name: &str, value: Option<&'a TaskDateArg>) -> Result<Option<&'a TaskDateValue>> {
    match value {
        None => Ok(None),
        Some(TaskDateArg::Set(date)) => Ok(Some(date)),
        Some(TaskDateArg::Clear) => bail!("{name} cannot be cleared when creating a task."),
    }
}

fn add_title(spec: &TaskModifierSpec) -> Result<&str> {
    spec.title
        .as_deref()
        .or(spec.text.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| anyhow::anyhow!("PKMS task creation requires task text or title:"))
}

fn add_priority(spec: &TaskModifierSpec) -> Result<String> {
    match spec.priority {
        None => Ok(String::new()),
        Some(TaskPriorityArg::Set(priority)) => Ok(format!(" [#{priority}]")),
        Some(TaskPriorityArg::Clear) => bail!("Invalid priority 'none'. Use A, B, or C."),
    }
}

fn canonical_state(config: &PkmsTaskConfig, requested_state: &str) -> Result<OrgTodoState> {
    config
        .task_states
        .valid_states
        .iter()
        .find(|state| state.eq_ignore_ascii_case(requested_state))
        .map(|state| OrgTodoState::new(state.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown TODO state '{}'. Valid states: {}",
                requested_state,
                config.task_states.valid_states.join(", ")
            )
        })
}

fn task_title_in_graph(graph: &Graph, path: &str, line_number: usize) -> Option<String> {
    let path = Path::new(path);
    graph
        .results
        .iter()
        .find(|result| result.path == path)?
        .parsed
        .headings
        .iter()
        .find(|heading| heading.line_number == line_number)
        .map(|heading| heading.title.clone())
}

fn pkms_task_location(location: GraphTaskLocation) -> pkms::TaskLocation {
    pkms::TaskLocation {
        path: location.path.into(),
        line_number: location.line_number,
    }
}
