use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::tasks::clock::TaskClock;
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskPriority, TaskProperty, TaskState};
use crate::tasks::modifiers::{
    TaskModifierSpec, TaskPriorityArg, org_date, validate_pkms_task_date_arg_on,
};
use crate::tasks::pkms::{self, PkmsInboxTarget};
use crate::tasks::pkms_mutation;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::super::render;
use super::{mod_date, mod_optional_text, mod_title, parse_mutation_due_date, validate_mod_source};

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
        scheduled: mod_date("schedule", spec.due.as_deref(), clock.today)?,
        deadline: mod_date("deadline", spec.deadline.as_deref(), clock.today)?,
        project: mod_optional_text(spec.project.as_deref()),
        description: spec
            .description
            .as_deref()
            .map(|description| description.trim().to_string()),
    };

    let graph = Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    let mut path = PathBuf::from(path);
    let mut line_number = line_number;
    let mut changes = Vec::new();

    if let Some(dependency) = mod_dependency(spec)? {
        match dependency {
            DependencyMod::Set(target_id) => {
                if target_id == canonical_id {
                    bail!("Cannot make a task depend on itself.");
                }
                let (target_path, target_line_number) =
                    graph.resolve_canonical_task_id(config, target_id)?;
                (path, line_number) = pkms::move_subtree_to_dependency(
                    &path,
                    line_number,
                    Path::new(&target_path),
                    target_line_number,
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
    let Some(raw) = spec.dependency.as_deref() else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(Some(DependencyMod::Clear));
    }
    let task_id = raw.parse::<TaskId>()?;
    let TaskId::Pkms(canonical_id) = task_id else {
        bail!("dep is available only for PKMS task IDs.");
    };
    Ok(Some(DependencyMod::Set(canonical_id)))
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
        .find(|(_, task_path, task_line)| task_path == &path && *task_line == parent.line_number)
        .map(|(id, _, line)| (id, line))
}

fn mod_pkms_priority(spec: &TaskModifierSpec) -> Result<Option<Option<TaskPriority>>> {
    let Some(priority) = spec.priority else {
        return Ok(None);
    };
    Ok(Some(match priority {
        TaskPriorityArg::Clear => None,
        TaskPriorityArg::Set(priority) => Some(priority),
    }))
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
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    let title = task_title_in_graph(&graph, &path, line_number).with_context(|| {
        format!("Resolved task but could not find title at {path}:{line_number}")
    })?;
    let output = pkms_mutation::replace_heading_state(&path, line_number, &new_state, dry_run)?;
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

    let (path, line_number) = if let Some(parent_id) = spec.dependency.as_deref() {
        add_dependency_task(config, spec, clock, parent_id)?
    } else {
        add_inbox_task(config, spec, clock)?
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
    let entry = format_task_entry(config, spec, clock, heading_level)?;
    pkms::append_inbox_entry(&inbox_target, &entry)
}

fn add_dependency_task(
    config: &ResolvedConfig,
    spec: &TaskModifierSpec,
    clock: TaskClock,
    parent_id: &str,
) -> Result<(PathBuf, usize)> {
    let task_id = parent_id.parse::<TaskId>()?;
    let TaskId::Pkms(canonical_id) = task_id else {
        bail!("dep is available only for PKMS task IDs.");
    };
    let graph = Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    let path = PathBuf::from(path);
    let parent_level = pkms::heading_level_at(&path, line_number)?;
    let entry = format_task_entry(config, spec, clock, parent_level + 1)?;
    pkms::append_child_entry(&path, line_number, &entry)
}

fn format_task_entry(
    config: &ResolvedConfig,
    spec: &TaskModifierSpec,
    clock: TaskClock,
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
    let due = validate_pkms_task_date_arg_on("due", spec.due.as_deref(), clock.today)?;
    let deadline =
        validate_pkms_task_date_arg_on("deadline", spec.deadline.as_deref(), clock.today)?;
    if due.is_some() || deadline.is_some() {
        let mut planning = Vec::new();
        if let Some(due) = due {
            planning.push(format!("SCHEDULED: {}", org_date(&due)?));
        }
        if let Some(deadline) = deadline {
            planning.push(format!("DEADLINE: {}", org_date(&deadline)?));
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
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    pkms_mutation::update_recurring_planning_date(&path, line_number, &date)?;
    let item = pkms::find_task_item_on(config, Path::new(&path), line_number, clock)?
        .with_context(|| {
            format!("Changed task but could not reload it from {path}:{line_number}")
        })?;
    render::print_mutation_output(ctx, "postpone", item)
}
