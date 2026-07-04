use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use crate::tasks::clock::TaskClock;
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskPriority, TaskProperty};
use crate::tasks::modifiers::{
    TaskDateArg, TaskDependencyArg, TaskModifierSpec, TaskPriorityArg, is_clear_value,
};
use crate::tasks::pkms;
use anyhow::{Context, Result, bail};
use pkms_org::Graph;
use pkms_org::graph::tasks::TaskLocation as GraphTaskLocation;
use pkms_org::org_task_mutation::{self, Change, OrgTaskProperty};
use pkms_org::parser::{OrgPriority, OrgTodoState};
use pkms_task::mutation::{add_pkms_task, postpone_pkms_task, set_pkms_state};
use std::process::ExitCode;

use super::super::render;
use super::{mod_title, validate_mod_source};

fn pkms_task_location(location: GraphTaskLocation) -> pkms::TaskLocation {
    pkms::TaskLocation {
        path: location.path.into(),
        line_number: location.line_number,
    }
}

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

    let graph = Graph::load(&config.org_config())?;
    let location = graph.resolve_canonical_task_id(&config.task_state_config(), canonical_id)?;
    let mut location = pkms_task_location(location);
    let mut changes = Vec::new();

    if let Some(dependency) = mod_dependency(spec)? {
        match dependency {
            DependencyMod::Set(target_id) => {
                if target_id == canonical_id {
                    bail!("Cannot make a task depend on itself.");
                }
                let target_location =
                    graph.resolve_canonical_task_id(&config.task_state_config(), target_id)?;
                let target_location = pkms_task_location(target_location);
                location = pkms::move_subtree_to_dependency(&location, &target_location)?;
                changes.push(render::TaskModChange {
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
        org_task_mutation::update_heading_properties(
            &location.path.display().to_string(),
            location.line_number,
            &modifier,
        )?
        .into_iter()
        .map(|change| render::TaskModChange {
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
    let todo_states = config.todo_states();
    let mut child_level = source.level;
    let parent = result.parsed.headings.iter().rev().find(|heading| {
        if heading.line_number >= location.line_number || heading.level >= child_level {
            return false;
        }
        child_level = heading.level;
        heading.todo_state.as_ref().is_some_and(|state| {
            todo_states
                .iter()
                .any(|todo_state| todo_state.eq_ignore_ascii_case(state))
        })
    })?;
    let path = location.path.display().to_string();
    graph
        .all_task_entries(&config.task_state_config())
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

fn mod_state(config: &ResolvedConfig, spec: &TaskModifierSpec) -> Result<Option<OrgTodoState>> {
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
    let output = set_pkms_state(
        &config.pkms_task_config(),
        canonical_id,
        requested_state,
        dry_run,
    )?;
    render::print_state_change(ctx, &output)
}

fn canonical_state(config: &ResolvedConfig, requested_state: &str) -> Result<OrgTodoState> {
    let states = config.todo_states();
    states
        .iter()
        .find(|state| state.eq_ignore_ascii_case(requested_state))
        .map(|state| OrgTodoState::new(state.clone()))
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
    let item = add_pkms_task(&config.pkms_task_config(), spec, clock)?;
    render::print_add_output(ctx, item)
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

pub(super) fn postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    to: &str,
    clock: TaskClock,
) -> Result<()> {
    let item = postpone_pkms_task(&config.pkms_task_config(), canonical_id, to, clock)?;
    render::print_mutation_output(ctx, "postpone", item)
}
