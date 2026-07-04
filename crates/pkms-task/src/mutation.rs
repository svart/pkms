use crate::clock::TaskClock;
use crate::config::PkmsTaskConfig;
use crate::id::TaskId;
use crate::model::{TaskDateValue, TaskItem, TaskPriority, TaskProperty};
use crate::modifiers::{
    TaskDateArg, TaskDependencyArg, TaskModifierSpec, TaskPriorityArg, is_clear_value,
    parse_task_date_arg_on,
};
use crate::pkms::{self, PkmsInboxTarget};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use pkms_org::Graph;
use pkms_org::graph::tasks::TaskLocation as GraphTaskLocation;
use pkms_org::org_task_edit::OrgTaskInsertSpec;
use pkms_org::org_task_mutation::{self, Change, OrgTaskProperty};
use pkms_org::parser::{OrgPriority, OrgTodoState};
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

#[derive(Debug, Serialize)]
pub struct TaskModChange {
    pub property: TaskProperty,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskModOutput {
    pub changed: bool,
    pub id: String,
    pub changes: Vec<TaskModChange>,
    pub item: Option<TaskItem>,
}

pub fn mod_pkms_task(
    config: &PkmsTaskConfig,
    canonical_id: usize,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<TaskModOutput> {
    validate_mod_source(spec, "pkms")?;
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }

    let title = mod_title(spec)?;
    let modifier = org_task_mutation::HeadingMod {
        state: mod_state(config, spec)?,
        title,
        priority: mod_pkms_priority(spec)?,
        tags: spec.labels.clone(),
        scheduled: mod_pkms_date(spec.due.as_ref()),
        deadline: mod_pkms_date(spec.deadline.as_ref()),
        project: mod_pkms_optional_text(spec.project.as_deref()),
        description: spec
            .description
            .as_deref()
            .map(|description| description.trim().to_string()),
    };

    let graph = Graph::load(&config.org)?;
    let location = graph.resolve_canonical_task_id(&config.task_states, canonical_id)?;
    let mut location = pkms_task_location(location);
    let mut changes = Vec::new();

    if let Some(dependency) = mod_dependency(spec)? {
        match dependency {
            DependencyMod::Set(target_id) => {
                if target_id == canonical_id {
                    bail!("Cannot make a task depend on itself.");
                }
                let target_location =
                    graph.resolve_canonical_task_id(&config.task_states, target_id)?;
                let target_location = pkms_task_location(target_location);
                location = pkms::move_subtree_to_dependency(&location, &target_location)?;
                changes.push(TaskModChange {
                    property: TaskProperty::Dependency,
                    old: None,
                    new: Some(TaskId::Pkms(target_id).display_id()),
                });
            }
            DependencyMod::Clear => {
                if let Some((parent_id, parent_location)) =
                    current_dependency_parent(&graph, config, &location)
                {
                    location = pkms::remove_subtree_dependency(&location, &parent_location)?;
                    changes.push(TaskModChange {
                        property: TaskProperty::Dependency,
                        old: Some(TaskId::Pkms(parent_id).display_id()),
                        new: None,
                    });
                }
            }
        }
    }

    changes.extend(
        org_task_mutation::update_heading_properties(
            &location.path.display().to_string(),
            location.line_number,
            &modifier,
        )?
        .into_iter()
        .map(|change| TaskModChange {
            property: task_property_from_org(change.property),
            old: change.old,
            new: change.new,
        }),
    );

    let item = if changes.is_empty() {
        None
    } else {
        Some(
            pkms::find_task_item_on(config, &location.path, location.line_number, clock)?
                .with_context(|| {
                    format!(
                        "Changed task but could not reload it from {}:{}",
                        location.path.display(),
                        location.line_number
                    )
                })?,
        )
    };
    Ok(TaskModOutput {
        changed: !changes.is_empty(),
        id: TaskId::Pkms(canonical_id).display_id(),
        changes,
        item,
    })
}

enum DependencyMod {
    Set(usize),
    Clear,
}

fn mod_dependency(spec: &TaskModifierSpec) -> Result<Option<DependencyMod>> {
    match spec.dependency.as_ref() {
        None => Ok(None),
        Some(TaskDependencyArg::Clear) => Ok(Some(DependencyMod::Clear)),
        Some(TaskDependencyArg::Set(TaskId::Pkms(canonical_id))) => {
            Ok(Some(DependencyMod::Set(*canonical_id)))
        }
        Some(TaskDependencyArg::Set(_)) => bail!("dep is available only for PKMS task IDs."),
    }
}

fn current_dependency_parent(
    graph: &Graph,
    config: &PkmsTaskConfig,
    location: &pkms::TaskLocation,
) -> Option<(usize, pkms::TaskLocation)> {
    let result = graph
        .results
        .iter()
        .find(|result| result.path == location.path.as_path())?;
    let source = result
        .parsed
        .headings
        .iter()
        .find(|heading| heading.line_number == location.line_number)?;
    let mut child_level = source.level;
    let parent = result.parsed.headings.iter().rev().find(|heading| {
        if heading.line_number >= location.line_number || heading.level >= child_level {
            return false;
        }
        child_level = heading.level;
        heading.todo_state.as_ref().is_some_and(|state| {
            config
                .task_states
                .valid_states
                .iter()
                .any(|todo_state| todo_state.eq_ignore_ascii_case(state))
        })
    })?;
    let path = location.path.display().to_string();
    graph
        .all_task_entries(&config.task_states)
        .into_iter()
        .find(|entry| {
            entry.path.as_str() == path.as_str() && entry.line_number == parent.line_number
        })
        .map(|entry| {
            (
                entry.id,
                pkms::TaskLocation {
                    path: location.path.clone(),
                    line_number: entry.line_number,
                },
            )
        })
}

fn mod_pkms_priority(spec: &TaskModifierSpec) -> Result<Change<OrgPriority>> {
    let Some(priority) = spec.priority else {
        return Ok(Change::Unchanged);
    };
    Ok(match priority {
        TaskPriorityArg::Clear => Change::Clear,
        TaskPriorityArg::Set(priority) => Change::Set(org_priority(priority)),
    })
}

fn mod_pkms_date(value: Option<&TaskDateArg>) -> Change<String> {
    match value {
        None => Change::Unchanged,
        Some(TaskDateArg::Clear) => Change::Clear,
        Some(TaskDateArg::Set(date)) => Change::Set(date.as_str().to_string()),
    }
}

fn mod_pkms_optional_text(value: Option<&str>) -> Change<String> {
    let Some(value) = value else {
        return Change::Unchanged;
    };
    let value = value.trim();
    if is_clear_value(value) {
        Change::Clear
    } else {
        Change::Set(value.to_string())
    }
}

fn mod_state(config: &PkmsTaskConfig, spec: &TaskModifierSpec) -> Result<Option<OrgTodoState>> {
    spec.state
        .as_deref()
        .map(|state| canonical_state(config, state))
        .transpose()
}

fn org_priority(priority: TaskPriority) -> OrgPriority {
    match priority {
        TaskPriority::A => OrgPriority::A,
        TaskPriority::B => OrgPriority::B,
        TaskPriority::C => OrgPriority::C,
    }
}

fn task_property_from_org(property: OrgTaskProperty) -> TaskProperty {
    match property {
        OrgTaskProperty::Status => TaskProperty::Status,
        OrgTaskProperty::Title => TaskProperty::Title,
        OrgTaskProperty::Priority => TaskProperty::Priority,
        OrgTaskProperty::Tags => TaskProperty::Tags,
        OrgTaskProperty::Scheduled => TaskProperty::Scheduled,
        OrgTaskProperty::Deadline => TaskProperty::Deadline,
        OrgTaskProperty::Project => TaskProperty::Project,
        OrgTaskProperty::Description => TaskProperty::Description,
    }
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
    let task = task_insert_spec(config, spec, heading_level)?;
    pkms::append_inbox_task(&inbox_target, &task)
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
    let task = task_insert_spec(config, spec, parent_level + 1)?;
    pkms::append_child_task(&location, &task)
}

fn task_insert_spec(
    config: &PkmsTaskConfig,
    spec: &TaskModifierSpec,
    heading_level: usize,
) -> Result<OrgTaskInsertSpec> {
    let title = add_title(spec)?.to_string();
    let state = match spec.state.as_deref() {
        Some(state) => canonical_state(config, state)?,
        None => OrgTodoState::new(
            config
                .task_states
                .open_states
                .first()
                .cloned()
                .unwrap_or_else(|| "TODO".to_string()),
        ),
    };
    let priority = add_priority(spec)?;
    let scheduled = add_date("due", spec.due.as_ref())?.map(|date| date.as_str().to_string());
    let deadline =
        add_date("deadline", spec.deadline.as_ref())?.map(|date| date.as_str().to_string());
    let description = spec
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string);

    Ok(OrgTaskInsertSpec {
        level: heading_level,
        state,
        title,
        priority,
        tags: spec.labels().to_vec(),
        scheduled,
        deadline,
        description,
    })
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

fn add_priority(spec: &TaskModifierSpec) -> Result<Option<OrgPriority>> {
    match spec.priority {
        None => Ok(None),
        Some(TaskPriorityArg::Set(priority)) => Ok(Some(org_priority(priority))),
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
