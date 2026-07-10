#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
use crate::cli::{TaskAgendaArgs, TaskCommand, TaskListArgs, TaskShortcutArgs};
use crate::command_context::CommandContext;
use crate::commands::open::{OpenOptions, OpenTarget};
use crate::commands::show::{HeadingTarget, ShowOptions, TaskIdEntry};
#[cfg(feature = "todoist")]
use crate::commands::task_common::RowSeparatorMode;
use crate::config::{ResolvedConfig, TaskCommandConfig};
use crate::output::{OutputContext, terminal_markup};
use anyhow::{Result, anyhow, bail};
#[cfg(feature = "todoist")]
use pkms_task::SourceSelection;
use pkms_task::{
    CanonicalTaskEntry, TaskClock, TaskId, TaskListItems, TaskLocation, TaskModifierSpec,
    TaskSourceKind,
};
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

mod id_command;
mod mutations;
mod plan;
mod providers;
mod render;

use mutations::{run_add, run_done, run_postpone, run_state, unsupported_task_source};
use plan::{
    ShortcutKind, TaskListMode, plan_agenda_request, plan_task_list_request, split_task_list_mode,
};

pub fn run(ctx: &CommandContext<'_>, command: &TaskCommand) -> Result<ExitCode> {
    let config = ctx.config();
    let output = ctx.output();
    let task_config = config.task_command_config();
    let task_id_snapshot = task_id_snapshot_before_command(&task_config, command);
    let clock = TaskClock::now();
    let runtime = TaskRuntime {
        config,
        output,
        clock,
    };
    let exit_code = match command {
        TaskCommand::List(args) => success(run_list(runtime, args)),
        TaskCommand::Agenda(args) => success(run_agenda(runtime, args)),
        TaskCommand::Inbox(args) => success(run_shortcut(runtime, args, ShortcutKind::Inbox)),
        TaskCommand::Show(args) => success(run_show(ctx, &args.id)),
        TaskCommand::Open(args) => success(run_open(ctx, &args.id, &args.editor, args.line)),
        TaskCommand::State(args) => {
            success(run_state(runtime, &args.id, &args.state, args.dry_run))
        }
        TaskCommand::Done(args) => success(run_done(runtime, &args.id, args.dry_run)),
        TaskCommand::Add(args) => success(run_add(runtime, &args.text)),
        TaskCommand::Postpone(args) => success(run_postpone(runtime, &args.id, &args.to)),
        TaskCommand::Target(args) => id_command::run(ctx, args, runtime),
    }?;
    maybe_warn_task_ids_changed(&task_config, task_id_snapshot);
    Ok(exit_code)
}

fn success(result: Result<()>) -> Result<ExitCode> {
    result.map(|()| ExitCode::SUCCESS)
}

#[derive(Clone, Copy)]
pub(super) struct TaskRuntime<'a> {
    config: &'a ResolvedConfig,
    output: &'a OutputContext,
    clock: TaskClock,
}

fn run_list(runtime: TaskRuntime<'_>, args: &TaskListArgs) -> Result<()> {
    let (mode, filters) = split_task_list_mode(&args.filters);
    match mode {
        TaskListMode::Tasks => {
            let task_config = runtime.config.task_command_config();
            let planned = plan_task_list_request(&task_config, args, &filters, runtime.clock)?;
            let task_config = runtime.config.pkms_task_config();
            let output =
                pkms_task::execute_task_list(runtime.config, &task_config, &planned.execution)?;
            render_task_list(runtime.output, output, &planned.table)
        }
        TaskListMode::Projects => run_projects(runtime, &filters),
        TaskListMode::Tags => run_tags(runtime, &filters),
    }
}

fn render_task_list(
    ctx: &OutputContext,
    output: pkms_task::TaskListExecution,
    table: &plan::TaskTableOptions,
) -> Result<()> {
    match output.items {
        TaskListItems::Flat { items, limit } => render::print_task_items(
            ctx,
            output.source,
            items,
            limit,
            render::TaskTableRenderOptions {
                columns: table.columns.as_deref(),
                row_separators: table.row_separators,
            },
        ),
        TaskListItems::Grouped {
            group_field,
            groups,
            total,
        } => render::print_grouped_task_items(
            ctx,
            output.source,
            group_field,
            groups,
            total,
            render::TaskTableRenderOptions {
                columns: table.columns.as_deref(),
                row_separators: table.row_separators,
            },
        ),
    }
}

fn run_shortcut(
    runtime: TaskRuntime<'_>,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<()> {
    let task_config = runtime.config.pkms_task_config();
    let (source, items) = pkms_task::collect_shortcut_items_on(
        runtime.config,
        &task_config,
        &args.filters,
        plan::shortcut_task_view(kind),
        runtime.clock,
    )?;
    let task_config = runtime.config.task_command_config();
    let columns = plan::resolve_task_table_columns(
        &task_config,
        source,
        plan::shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(
        runtime.output,
        source,
        items,
        args.limit,
        render::TaskTableRenderOptions {
            columns: columns.as_deref(),
            row_separators: args.table.line_sep.into(),
        },
    )
}

fn run_agenda(runtime: TaskRuntime<'_>, args: &TaskAgendaArgs) -> Result<()> {
    let task_config = runtime.config.task_command_config();
    let planned = plan_agenda_request(&task_config, args, runtime.clock)?;
    let task_config = runtime.config.pkms_task_config();
    let output = pkms_task::execute_task_agenda(runtime.config, &task_config, &planned.execution)?;
    render_task_agenda(runtime.output, output, runtime.clock.now, &planned.table)
}

fn render_task_agenda(
    ctx: &OutputContext,
    output: pkms_task::AgendaExecution,
    now: chrono::NaiveTime,
    table: &plan::TaskTableOptions,
) -> Result<()> {
    render::print_agenda_task_items(
        ctx,
        output.source,
        output.items,
        output.limit,
        render::AgendaTaskRenderOptions {
            table: render::TaskTableRenderOptions {
                columns: table.columns.as_deref(),
                row_separators: table.row_separators,
            },
            today: output.today,
            now,
            window: output.window,
        },
    )
}

pub(super) fn run_show(ctx: &CommandContext<'_>, id: &str) -> Result<()> {
    let config = ctx.config();
    let output = ctx.output();
    match id.parse::<TaskId>()? {
        TaskId::Pkms(id) => {
            let task_config = config.task_command_config();
            let entries = load_canonical_task_entries(&task_config)?;
            let location = resolve_task_location_from_entries(&entries, id)?;
            crate::commands::show::run(
                ctx,
                &ShowOptions {
                    targets: vec![HeadingTarget::Location {
                        path: location.path.into(),
                        line_number: location.line_number,
                    }],
                    task_ids: show_task_id_entries(&entries),
                },
            )
        }
        TaskId::Todoist(id) => show_todoist_task(config, output, &id),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(super) fn run_open(
    ctx: &CommandContext<'_>,
    id: &str,
    editor: &str,
    line: Option<usize>,
) -> Result<()> {
    let config = ctx.config();
    match id.parse::<TaskId>()? {
        TaskId::Pkms(id) => {
            let task_config = config.task_command_config();
            let entries = load_canonical_task_entries(&task_config)?;
            let location = resolve_task_location_from_entries(&entries, id)?;
            crate::commands::open::run(
                ctx,
                &OpenOptions {
                    targets: vec![OpenTarget::Location {
                        path: location.path.into(),
                        line_number: location.line_number,
                    }],
                    editor: editor.to_string(),
                    line,
                },
            )
        }
        TaskId::Todoist(_) => bail!("Todoist task source is not implemented yet"),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

#[cfg(feature = "todoist")]
fn show_todoist_task(config: &ResolvedConfig, ctx: &OutputContext, id: &str) -> Result<()> {
    let item = pkms_task::get_todoist_item(&providers::todoist_config(config)?, id)?;
    match ctx.format {
        OutputFormat::Text => render::print_task_table(
            &[item],
            1,
            SourceSelection::Todoist,
            render::TaskTableRenderOptions {
                columns: None,
                row_separators: RowSeparatorMode::Off,
            },
        ),
        OutputFormat::Json => ctx.print_json(&item),
        OutputFormat::Ndjson => ctx.print_ndjson(&[item]),
    }
}

#[cfg(not(feature = "todoist"))]
fn show_todoist_task(_config: &ResolvedConfig, _ctx: &OutputContext, _id: &str) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

fn run_projects(runtime: TaskRuntime<'_>, filters: &[String]) -> Result<()> {
    let rows = providers::collect_task_metadata(
        runtime.config,
        filters,
        providers::MetadataKind::Projects,
        runtime.clock,
    )?;
    render::print_metadata_rows(runtime.output, "project", &rows)
}

fn run_tags(runtime: TaskRuntime<'_>, filters: &[String]) -> Result<()> {
    let rows = providers::collect_task_metadata(
        runtime.config,
        filters,
        providers::MetadataKind::Tags,
        runtime.clock,
    )?;
    render::print_metadata_rows(runtime.output, "tag", &rows)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskIdSnapshot(Vec<TaskIdentity>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskIdentity {
    path: String,
    ordinal: usize,
}

impl TaskIdSnapshot {
    fn capture(config: &TaskCommandConfig) -> Result<Self> {
        let entries = load_canonical_task_entries(config)?;
        let identities = task_identities_by_location(&entries);
        let mut ordered = Vec::new();
        for entry in entries {
            let key = (entry.path.clone(), entry.line_number);
            let identity = identities.get(&key).ok_or_else(|| {
                anyhow!(
                    "Task ID snapshot missing task identity for {}:{}",
                    entry.path,
                    entry.line_number
                )
            })?;
            ordered.push(identity.clone());
        }
        Ok(Self(ordered))
    }
}

fn load_canonical_task_entries(config: &TaskCommandConfig) -> Result<Vec<CanonicalTaskEntry>> {
    let graph = pkms_org::Graph::load(&config.org)?;
    Ok(pkms_task::all_task_entries(&config.task_states, &graph))
}

fn resolve_task_location_from_entries(
    entries: &[CanonicalTaskEntry],
    id: usize,
) -> Result<TaskLocation> {
    if id == 0 || id > entries.len() {
        anyhow::bail!(
            "No task with canonical ID {}. Valid range is 1-{}",
            id,
            entries.len()
        );
    }
    let entry = &entries[id - 1];
    Ok(TaskLocation {
        path: entry.path.clone(),
        line_number: entry.line_number,
    })
}

fn show_task_id_entries(entries: &[CanonicalTaskEntry]) -> Vec<TaskIdEntry> {
    entries
        .iter()
        .map(|entry| TaskIdEntry {
            id: entry.id,
            path: entry.path.clone(),
            line_number: entry.line_number,
        })
        .collect()
}

fn task_identities_by_location(
    entries: &[CanonicalTaskEntry],
) -> HashMap<(String, usize), TaskIdentity> {
    let mut line_numbers_by_path: HashMap<&str, Vec<usize>> = HashMap::new();
    for entry in entries {
        line_numbers_by_path
            .entry(entry.path.as_str())
            .or_default()
            .push(entry.line_number);
    }

    let mut identities = HashMap::new();
    for (path, line_numbers) in &mut line_numbers_by_path {
        line_numbers.sort_unstable();
        for (index, line_number) in line_numbers.iter().enumerate() {
            let path = (*path).to_string();
            identities.insert(
                (path.clone(), *line_number),
                TaskIdentity {
                    path,
                    ordinal: index + 1,
                },
            );
        }
    }

    identities
}

fn task_id_snapshot_before_command(
    config: &TaskCommandConfig,
    command: &TaskCommand,
) -> Option<TaskIdSnapshot> {
    if !command_may_change_pkms_task_ids(command) {
        return None;
    }
    match TaskIdSnapshot::capture(config) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            tracing::debug!(error = %err, "could not snapshot task IDs before command");
            None
        }
    }
}

fn maybe_warn_task_ids_changed(config: &TaskCommandConfig, before: Option<TaskIdSnapshot>) {
    let Some(before) = before else {
        return;
    };
    match TaskIdSnapshot::capture(config) {
        Ok(after) if before != after => print_task_id_change_warning(),
        Ok(_) => {}
        Err(err) => {
            tracing::debug!(error = %err, "could not snapshot task IDs after command");
        }
    }
}

fn print_task_id_change_warning() {
    let _ = io::stdout().flush();
    eprintln!(
        "{}",
        terminal_markup::format_stderr_warning(
            "WARN: Task IDs changed; run `pkms task list` before using task IDs again."
        )
    );
}

fn command_may_change_pkms_task_ids(command: &TaskCommand) -> bool {
    match command {
        TaskCommand::State(args) => !args.dry_run && is_pkms_task_id(&args.id),
        TaskCommand::Done(args) => !args.dry_run && is_pkms_task_id(&args.id),
        TaskCommand::Add(args) => add_may_write_pkms_task(args),
        TaskCommand::Postpone(args) => is_pkms_task_id(&args.id),
        TaskCommand::Target(args) => target_may_write_pkms_task(args),
        TaskCommand::List(_)
        | TaskCommand::Agenda(_)
        | TaskCommand::Inbox(_)
        | TaskCommand::Show(_)
        | TaskCommand::Open(_) => false,
    }
}

fn add_may_write_pkms_task(args: &crate::cli::TaskAddArgs) -> bool {
    TaskModifierSpec::parse(&args.text)
        .is_ok_and(|spec| matches!(spec.source_or_default(), TaskSourceKind::Pkms))
}

fn target_may_write_pkms_task(args: &[String]) -> bool {
    let Some((id, rest)) = args.split_first() else {
        return false;
    };
    if !is_pkms_task_id(id) {
        return false;
    }
    matches!(
        rest.first().map(String::as_str),
        Some("state" | "done" | "postpone" | "mod")
    )
}

fn is_pkms_task_id(id: &str) -> bool {
    id.parse::<TaskId>()
        .is_ok_and(|id| matches!(id, TaskId::Pkms(_)))
}
