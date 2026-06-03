#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
use crate::cli::{TaskAddArgs, TaskDoneArgs, TaskModArgs, TaskPostponeArgs, TaskStateArgs};
use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use crate::tasks::add::{
    TaskAddSpec, org_date, parse_add_date_arg_on, pkms_priority, validate_pkms_date_arg_on,
};
use crate::tasks::clock::TaskClock;
use crate::tasks::id::TaskId;
use crate::tasks::pkms::{self, PkmsInboxTarget};
use crate::tasks::pkms_mutation;
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
#[cfg(feature = "todoist")]
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::render;

pub(in crate::commands::task) fn run_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskStateArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(_) => set_pkms_task_state(config, ctx, &args.id, &args.state, args.dry_run),
        TaskId::Todoist(id) => set_todoist_task_state(config, ctx, &id, &args.state, args.dry_run),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(in crate::commands::task) fn run_done(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskDoneArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Todoist(id) => return close_todoist_task(config, ctx, &id, args.dry_run),
        TaskId::External { source, .. } => return unsupported_task_source(&source),
        TaskId::Pkms(_) => {}
    }
    let closed_state = config
        .closed_todo_states()
        .first()
        .cloned()
        .unwrap_or_else(|| "DONE".to_string());
    set_pkms_task_state(config, ctx, &args.id, &closed_state, args.dry_run)
}

pub(in crate::commands::task) fn run_add(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskAddArgs,
) -> Result<()> {
    let spec = TaskAddSpec::parse(&args.text)?;
    let clock = TaskClock::now();
    if spec.source.eq_ignore_ascii_case("pkms") {
        return add_pkms_task(config, ctx, &spec, clock);
    }
    if spec.source.eq_ignore_ascii_case("todoist") {
        return add_todoist_task(config, ctx, &spec, clock);
    }
    bail!(
        "Unknown task source '{}'. Use pkms or todoist.",
        spec.source
    )
}

pub(in crate::commands::task) fn run_postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskPostponeArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => {
            postpone_pkms_recurring_task(config, ctx, canonical_id, &args.to, TaskClock::now())
        }
        TaskId::Todoist(id) => {
            postpone_todoist_recurring_task(config, ctx, &id, &args.to, TaskClock::now())
        }
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(in crate::commands::task) fn run_mod(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskModArgs,
) -> Result<ExitCode> {
    let spec = TaskAddSpec::parse(&args.modifiers)?;
    let clock = TaskClock::now();
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => mod_pkms_task(config, ctx, canonical_id, &spec, clock),
        TaskId::Todoist(id) => mod_todoist_task(config, ctx, &id, &spec, clock),
        TaskId::External { source, .. } => {
            unsupported_task_source(&source).map(|()| ExitCode::SUCCESS)
        }
    }
}

fn mod_pkms_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    spec: &TaskAddSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
    validate_mod_source(spec, "pkms")?;
    if spec.provided.note {
        bail!("note is available only for PKMS task creation.");
    }

    let title = mod_title(spec)?;
    let modifier = pkms_mutation::HeadingMod {
        title,
        priority: mod_pkms_priority(spec)?,
        tags: spec.provided.labels.then(|| spec.labels.clone()),
        scheduled: mod_date(
            "schedule",
            spec.provided.due,
            spec.due.as_deref(),
            clock.today,
        )?,
        deadline: mod_date(
            "deadline",
            spec.provided.deadline,
            spec.deadline.as_deref(),
            clock.today,
        )?,
        project: mod_optional_text(spec.provided.project, spec.project.as_deref()),
        description: spec.provided.description.then(|| {
            spec.description
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string()
        }),
    };

    let graph = crate::graph::Graph::load(config)?;
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
                    property: "Dependency",
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
                        property: "Dependency",
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

fn mod_dependency(spec: &TaskAddSpec) -> Result<Option<DependencyMod>> {
    if !spec.provided.dependency {
        return Ok(None);
    }
    let raw = spec.dependency.as_deref().unwrap_or_default();
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
    graph: &crate::graph::Graph,
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

fn validate_mod_source(spec: &TaskAddSpec, expected: &str) -> Result<()> {
    if spec.provided.source && !spec.source.eq_ignore_ascii_case(expected) {
        bail!("Task source cannot be changed by task mod.");
    }
    Ok(())
}

fn mod_title(spec: &TaskAddSpec) -> Result<Option<String>> {
    if spec.provided.title && spec.provided.text {
        bail!("task mod uses title: or positional text, not both.");
    }
    Ok(spec
        .title
        .as_deref()
        .or(spec.text.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string))
}

fn mod_pkms_priority(spec: &TaskAddSpec) -> Result<Option<Option<char>>> {
    if !spec.provided.priority {
        return Ok(None);
    }
    Ok(Some(
        match spec.priority.as_deref().unwrap_or_default().trim() {
            "" | "none" | "None" | "NONE" => None,
            value => Some(pkms_priority(value)?),
        },
    ))
}

fn mod_optional_text(provided: bool, value: Option<&str>) -> Option<Option<String>> {
    provided.then(|| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
            .map(str::to_string)
    })
}

fn mod_date(
    name: &str,
    provided: bool,
    value: Option<&str>,
    today: NaiveDate,
) -> Result<Option<Option<String>>> {
    if !provided {
        return Ok(None);
    }
    let Some(value) = value.map(str::trim) else {
        return Ok(Some(None));
    };
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Some(None));
    }
    Ok(Some(Some(parse_add_date_arg_on(name, value, today)?)))
}

fn set_pkms_task_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    let task_id = id.parse::<TaskId>()?;
    let TaskId::Pkms(canonical_id) = task_id else {
        return match task_id {
            TaskId::Todoist(_) => bail!("Todoist task source is not implemented yet"),
            TaskId::External { source, .. } => unsupported_task_source(&source),
            TaskId::Pkms(_) => unreachable!(),
        };
    };
    let new_state = canonical_state(config, requested_state)?;
    let graph = crate::graph::Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    let output = pkms_mutation::replace_heading_state(&path, line_number, &new_state, dry_run)?;
    render::print_state_change(
        ctx,
        &render::TaskStateChangeOutput {
            id: TaskId::Pkms(canonical_id).display_id(),
            path: output.path,
            line_number: output.line_number,
            old_state: output.old_state,
            new_state: output.new_state,
            dry_run,
        },
    )
}

#[cfg(feature = "todoist")]
fn set_todoist_task_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    match requested_state.to_ascii_lowercase().as_str() {
        "done" => close_todoist_task(config, ctx, id, dry_run),
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
            let metadata = todoist_metadata_for_task(&client, &task)?;
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
fn set_todoist_task_state(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _requested_state: &str,
    _dry_run: bool,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

fn canonical_state(config: &ResolvedConfig, requested_state: &str) -> Result<String> {
    let states = config.todo_states();
    states
        .iter()
        .find(|state| state.eq_ignore_ascii_case(requested_state))
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown TODO state '{}'. Valid states: {}",
                requested_state,
                states.join(", ")
            )
        })
}

fn add_pkms_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskAddSpec,
    clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() && spec.note.is_some() {
        bail!("note and dep cannot be used together for PKMS task creation.");
    }

    let (path, line_number) = if let Some(parent_id) = spec.dependency.as_deref() {
        add_pkms_dependency_task(config, spec, clock, parent_id)?
    } else {
        add_pkms_inbox_task(config, spec, clock)?
    };
    let item = pkms::find_task_item_on(config, &path, line_number, clock)?.with_context(|| {
        format!(
            "Created task but could not reload it from {}",
            path.display()
        )
    })?;
    render::print_add_output(ctx, item)
}

fn add_pkms_inbox_task(
    config: &ResolvedConfig,
    spec: &TaskAddSpec,
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
    let entry = format_pkms_task_entry(config, spec, clock, heading_level)?;
    pkms::append_inbox_entry(&inbox_target, &entry)
}

fn add_pkms_dependency_task(
    config: &ResolvedConfig,
    spec: &TaskAddSpec,
    clock: TaskClock,
    parent_id: &str,
) -> Result<(PathBuf, usize)> {
    let task_id = parent_id.parse::<TaskId>()?;
    let TaskId::Pkms(canonical_id) = task_id else {
        bail!("dep is available only for PKMS task IDs.");
    };
    let graph = crate::graph::Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    let path = PathBuf::from(path);
    let parent_level = pkms::heading_level_at(&path, line_number)?;
    let entry = format_pkms_task_entry(config, spec, clock, parent_level + 1)?;
    pkms::append_child_entry(&path, line_number, &entry)
}

fn format_pkms_task_entry(
    config: &ResolvedConfig,
    spec: &TaskAddSpec,
    clock: TaskClock,
    heading_level: usize,
) -> Result<String> {
    let title = pkms_add_title(spec)?;
    let state = config
        .open_todo_states()
        .first()
        .cloned()
        .unwrap_or_else(|| "TODO".to_string());
    let priority = spec
        .priority
        .as_deref()
        .map(pkms_priority)
        .transpose()?
        .map(|priority| format!(" [#{priority}]"))
        .unwrap_or_default();
    let tags = if spec.labels.is_empty() {
        String::new()
    } else {
        format!(" :{}:", spec.labels.join(":"))
    };

    let level = "*".repeat(heading_level);
    let mut entry = format!("{level} {state}{priority} {title}{tags}\n");
    let due = validate_pkms_date_arg_on("due", spec.due.as_deref(), clock.today)?;
    let deadline = validate_pkms_date_arg_on("deadline", spec.deadline.as_deref(), clock.today)?;
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

fn pkms_add_title(spec: &TaskAddSpec) -> Result<&str> {
    spec.title
        .as_deref()
        .or(spec.text.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| anyhow::anyhow!("PKMS task creation requires task text or title:"))
}

fn postpone_pkms_recurring_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    to: &str,
    clock: TaskClock,
) -> Result<()> {
    let date = parse_mutation_due_date(to, clock.today)?;
    let graph = crate::graph::Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    pkms_mutation::update_recurring_planning_date(&path, line_number, &date)?;
    let item = pkms::find_task_item_on(config, Path::new(&path), line_number, clock)?
        .with_context(|| {
            format!("Changed task but could not reload it from {path}:{line_number}")
        })?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(feature = "todoist")]
fn add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskAddSpec,
    clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }
    if is_structured_add(spec) {
        return create_structured_todoist_task(config, ctx, spec, clock);
    }
    quick_add_todoist_task(config, ctx, spec)
}

#[cfg(not(feature = "todoist"))]
fn add_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    spec: &TaskAddSpec,
    _clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn is_structured_add(spec: &TaskAddSpec) -> bool {
    spec.title.is_some()
        || spec.due.is_some()
        || spec.deadline.is_some()
        || !spec.labels.is_empty()
        || spec.priority.is_some()
        || spec.description.is_some()
}

#[cfg(feature = "todoist")]
fn create_structured_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskAddSpec,
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
    let priority = spec
        .priority
        .as_deref()
        .map(todoist_create_priority)
        .transpose()?;
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
        labels: spec.labels.clone(),
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
fn quick_add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskAddSpec,
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
    let id = todoist_created_task_id(&response)?;
    let task = client.get_task(&id)?;
    let metadata = crate::tasks::todoist::TodoistMetadata::new(client.list_projects()?);
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, Some(&metadata));
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
fn mod_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    spec: &TaskAddSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
    validate_mod_source(spec, "todoist")?;
    if spec.provided.note {
        bail!("note is available only for PKMS task creation.");
    }
    if spec.provided.dependency {
        bail!("dep is available only for PKMS task creation.");
    }

    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let existing = client.get_task(id)?;
    let needs_metadata = existing.project_id.is_some() || spec.provided.project;
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
        push_todoist_change(
            &mut changes,
            "Title",
            Some(old_item.title.clone()),
            Some(title.clone()),
        );
        request.insert("content".to_string(), serde_json::Value::String(title));
    }
    if spec.provided.description {
        let new = spec
            .description
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string();
        push_todoist_change(
            &mut changes,
            "Description",
            old_item.body.clone(),
            (!new.is_empty()).then_some(new.clone()),
        );
        request.insert("description".to_string(), serde_json::Value::String(new));
    }
    if spec.provided.labels {
        let old = (!old_item.tags.is_empty()).then(|| old_item.tags.join(", "));
        let new = (!spec.labels.is_empty()).then(|| spec.labels.join(", "));
        push_todoist_change(&mut changes, "Tags", old, new);
        request.insert("labels".to_string(), serde_json::json!(spec.labels));
    }
    if spec.provided.priority {
        let priority = match spec.priority.as_deref().unwrap_or_default().trim() {
            "" | "none" | "None" | "NONE" => 1,
            value => todoist_create_priority(value)?,
        };
        push_todoist_change(
            &mut changes,
            "Priority",
            old_item.priority.clone(),
            todoist_priority_label(priority),
        );
        request.insert("priority".to_string(), serde_json::json!(priority));
    }
    if let Some(date) = mod_date(
        "schedule",
        spec.provided.due,
        spec.due.as_deref(),
        clock.today,
    )? {
        if old_item.scheduled_date_str() != date.as_deref() {
            let new_raw = date.as_deref().map(org_date).transpose()?;
            push_todoist_change(
                &mut changes,
                "Scheduled",
                old_item.scheduled.as_ref().map(|date| date.raw.clone()),
                new_raw,
            );
        }
        request.insert(
            "due_date".to_string(),
            date.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    }
    if let Some(date) = mod_date(
        "deadline",
        spec.provided.deadline,
        spec.deadline.as_deref(),
        clock.today,
    )? {
        if old_item.deadline_date_str() != date.as_deref() {
            let new_raw = date.as_deref().map(org_date).transpose()?;
            push_todoist_change(
                &mut changes,
                "Deadline",
                old_item.deadline.as_ref().map(|date| date.raw.clone()),
                new_raw,
            );
        }
        request.insert(
            "deadline_date".to_string(),
            date.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    }
    if let Some(project) = mod_optional_text(spec.provided.project, spec.project.as_deref()) {
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
        push_todoist_change(
            &mut changes,
            "Project",
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
    let metadata = todoist_metadata_for_task(&client, &task)?.or(metadata);
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

#[cfg(feature = "todoist")]
fn push_todoist_change(
    changes: &mut Vec<render::TaskModChange>,
    property: &'static str,
    old: Option<String>,
    new: Option<String>,
) {
    changes.push(render::TaskModChange { property, old, new });
}

#[cfg(feature = "todoist")]
fn todoist_priority_label(priority: u8) -> Option<String> {
    match priority {
        4 => Some("A".to_string()),
        3 => Some("B".to_string()),
        2 => Some("C".to_string()),
        _ => None,
    }
}

#[cfg(feature = "todoist")]
fn postpone_todoist_recurring_task(
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
    let metadata = todoist_metadata_for_task(&client, &task)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(not(feature = "todoist"))]
fn mod_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _spec: &TaskAddSpec,
    _clock: TaskClock,
) -> Result<ExitCode> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(not(feature = "todoist"))]
fn postpone_todoist_recurring_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _to: &str,
    _clock: TaskClock,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn todoist_metadata_for_task(
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
fn todoist_created_task_id(response: &serde_json::Value) -> Result<String> {
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
fn validate_date_arg(name: &str, value: Option<&str>, today: NaiveDate) -> Result<Option<String>> {
    value
        .map(|value| parse_add_date_arg_on(name, value, today))
        .transpose()
}

fn parse_mutation_due_date(value: &str, today: NaiveDate) -> Result<String> {
    parse_add_date_arg_on("due", value, today)
}

#[cfg(feature = "todoist")]
fn todoist_create_priority(value: &str) -> Result<u8> {
    match value.to_ascii_uppercase().as_str() {
        "A" => Ok(4),
        "B" => Ok(3),
        "C" => Ok(2),
        _ => bail!("Invalid priority '{value}'. Use A, B, or C."),
    }
}

#[cfg(feature = "todoist")]
fn close_todoist_task(
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
fn close_todoist_task(
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

pub(in crate::commands::task) fn unsupported_task_source(source: &str) -> Result<()> {
    bail!("Task source '{source}' is not configured in this build.")
}
