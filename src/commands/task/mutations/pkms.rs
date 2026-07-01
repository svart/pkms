use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::tasks::clock::TaskClock;
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDateValue, TaskPriority, TaskProperty, TaskState};
use crate::tasks::modifiers::{
    TaskDateArg, TaskDependencyArg, TaskModifierSpec, TaskPriorityArg, is_clear_value, org_date,
};
use crate::tasks::pkms::{self, PkmsInboxTarget};
use crate::tasks::pkms_mutation::{self, Change};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::super::render;
use super::{mod_title, parse_mutation_due_date, validate_mod_source};

pub(super) fn mod_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
    validate_mod_source(spec, "pkms")?;
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }

    let title = mod_title(spec)?;
    let modifier = pkms_mutation::HeadingMod {
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

    let graph = Graph::load(config)?;
    let location = graph.resolve_canonical_task_id(config, canonical_id)?;
    let mut path = PathBuf::from(location.path);
    let mut line_number = location.line_number;
    let mut changes = Vec::new();

    if let Some(dependency) = mod_dependency(spec)? {
        match dependency {
            DependencyMod::Set(target_id) => {
                if target_id == canonical_id {
                    bail!("Cannot make a task depend on itself.");
                }
                let target_location = graph.resolve_canonical_task_id(config, target_id)?;
                (path, line_number) = pkms::move_subtree_to_dependency(
                    &path,
                    line_number,
                    Path::new(&target_location.path),
                    target_location.line_number,
                )?;
                changes.push(render::TaskModChange {
                    property: TaskProperty::Dependency,
                    old: None,
                    new: Some(TaskId::Pkms(target_id).display_id()),
                });
            }
            DependencyMod::Clear => {
                if let Some((parent_id, parent_line_number)) =
                    current_dependency_parent(&graph, config, &path, line_number)
                {
                    (path, line_number) =
                        pkms::remove_subtree_dependency(&path, line_number, parent_line_number)?;
                    changes.push(render::TaskModChange {
                        property: TaskProperty::Dependency,
                        old: Some(TaskId::Pkms(parent_id).display_id()),
                        new: None,
                    });
                }
            }
        }
    }

    changes.extend(
        pkms_mutation::update_heading_properties(
            &path.display().to_string(),
            line_number,
            &modifier,
        )?
        .into_iter()
        .map(|change| render::TaskModChange {
            property: change.property,
            old: change.old,
            new: change.new,
        }),
    );

    let item = if changes.is_empty() {
        None
    } else {
        Some(
            pkms::find_task_item_on(config, &path, line_number, clock)?.with_context(|| {
                format!(
                    "Changed task but could not reload it from {}:{line_number}",
                    path.display()
                )
            })?,
        )
    };
    render::print_mod_output(
        ctx,
        render::TaskModOutput {
            changed: !changes.is_empty(),
            id: TaskId::Pkms(canonical_id).display_id(),
            changes,
            item,
        },
        clock.today,
    )
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
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
) -> Option<(usize, usize)> {
    let result = graph.results.iter().find(|result| result.path == path)?;
    let source = result
        .parsed
        .headings
        .iter()
        .find(|heading| heading.line_number == line_number)?;
    let todo_states = config.todo_states();
    let mut child_level = source.level;
    let parent = result.parsed.headings.iter().rev().find(|heading| {
        if heading.line_number >= line_number || heading.level >= child_level {
            return false;
        }
        child_level = heading.level;
        heading.todo_state.as_ref().is_some_and(|state| {
            todo_states
                .iter()
                .any(|todo_state| todo_state.eq_ignore_ascii_case(state))
        })
    })?;
    let path = path.display().to_string();
    graph
        .all_task_entries(config)
        .into_iter()
        .find(|entry| {
            entry.path.as_str() == path.as_str() && entry.line_number == parent.line_number
        })
        .map(|entry| (entry.id, entry.line_number))
}

fn mod_pkms_priority(spec: &TaskModifierSpec) -> Result<Change<TaskPriority>> {
    let Some(priority) = spec.priority else {
        return Ok(Change::Unchanged);
    };
    Ok(match priority {
        TaskPriorityArg::Clear => Change::Clear,
        TaskPriorityArg::Set(priority) => Change::Set(priority),
    })
}

fn mod_pkms_date(value: Option<&TaskDateArg>) -> Change<TaskDateValue> {
    match value {
        None => Change::Unchanged,
        Some(TaskDateArg::Clear) => Change::Clear,
        Some(TaskDateArg::Set(date)) => Change::Set(date.clone()),
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

fn mod_state(config: &ResolvedConfig, spec: &TaskModifierSpec) -> Result<Option<TaskState>> {
    spec.state
        .as_deref()
        .map(|state| canonical_state(config, state))
        .transpose()
}

pub(super) fn set_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    let new_state = canonical_state(config, requested_state)?;
    let graph = Graph::load(config)?;
    let location = graph.resolve_canonical_task_id(config, canonical_id)?;
    let title =
        task_title_in_graph(&graph, &location.path, location.line_number).with_context(|| {
            format!(
                "Resolved task but could not find title at {}:{}",
                location.path, location.line_number
            )
        })?;
    let output = pkms_mutation::replace_heading_state(
        &location.path,
        location.line_number,
        &new_state,
        dry_run,
    )?;
    render::print_state_change(
        ctx,
        &render::TaskStateChangeOutput {
            id: TaskId::Pkms(canonical_id).display_id(),
            title,
            path: output.path,
            line_number: output.line_number,
            old_state: output.old_state,
            new_state: output.new_state,
            dry_run,
        },
    )
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

fn canonical_state(config: &ResolvedConfig, requested_state: &str) -> Result<TaskState> {
    let states = config.todo_states();
    states
        .iter()
        .find(|state| state.eq_ignore_ascii_case(requested_state))
        .map(|state| TaskState::new(state.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown TODO state '{}'. Valid states: {}",
                requested_state,
                states.join(", ")
            )
        })
}

pub(super) fn add(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() && spec.note.is_some() {
        bail!("note and dep cannot be used together for PKMS task creation.");
    }

    let (path, line_number) = match spec.dependency.as_ref() {
        Some(TaskDependencyArg::Set(TaskId::Pkms(parent_id))) => {
            add_dependency_task(config, spec, *parent_id)?
        }
        Some(TaskDependencyArg::Set(_)) => bail!("dep is available only for PKMS task IDs."),
        Some(TaskDependencyArg::Clear) => bail!("dep requires a PKMS task ID for task creation."),
        None => add_inbox_task(config, spec, clock)?,
    };
    let item = pkms::find_task_item_on(config, &path, line_number, clock)?.with_context(|| {
        format!(
            "Created task but could not reload it from {}",
            path.display()
        )
    })?;
    render::print_add_output(ctx, item)
}

fn add_inbox_task(
    config: &ResolvedConfig,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<(PathBuf, usize)> {
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
    config: &ResolvedConfig,
    spec: &TaskModifierSpec,
    canonical_id: usize,
) -> Result<(PathBuf, usize)> {
    let graph = Graph::load(config)?;
    let location = graph.resolve_canonical_task_id(config, canonical_id)?;
    let path = PathBuf::from(location.path);
    let line_number = location.line_number;
    let parent_level = pkms::heading_level_at(&path, line_number)?;
    let entry = format_task_entry(config, spec, parent_level + 1)?;
    pkms::append_child_entry(&path, line_number, &entry)
}

fn format_task_entry(
    config: &ResolvedConfig,
    spec: &TaskModifierSpec,
    heading_level: usize,
) -> Result<String> {
    let title = add_title(spec)?;
    let state = match spec.state.as_deref() {
        Some(state) => canonical_state(config, state)?.to_string(),
        None => config
            .open_todo_states()
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

pub(super) fn postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    to: &str,
    clock: TaskClock,
) -> Result<()> {
    let date = parse_mutation_due_date(to, clock.today)?;
    let graph = Graph::load(config)?;
    let location = graph.resolve_canonical_task_id(config, canonical_id)?;
    pkms_mutation::update_recurring_planning_date(&location.path, location.line_number, &date)?;
    let item = pkms::find_task_item_on(
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
    })?;
    render::print_mutation_output(ctx, "postpone", item)
}
