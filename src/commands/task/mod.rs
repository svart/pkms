#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
use crate::cli::{
    TaskAgendaArgs, TaskCommand, TaskListArgs, TaskOpenArgs, TaskShortcutArgs, TaskTargetArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::config::ResolvedConfig;
use crate::output::{OutputContext, terminal_markup};
use crate::tasks::clock::TaskClock;
#[cfg(feature = "todoist")]
use crate::tasks::filter::SourceSelection;
use crate::tasks::filter::parse_task_filters;
use crate::tasks::id::TaskId;
use crate::tasks::model::TaskSourceKind;
use crate::tasks::modifiers::TaskModifierSpec;
use crate::tasks::provider::TaskMetadataRow;
use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

mod agenda;
mod execution;
mod id_command;
mod mutations;
mod plan;
mod providers;
mod render;
mod todo;

use execution::{AgendaExecution, TaskListExecution};
use mutations::{run_add, run_done, run_postpone, run_state, unsupported_task_source};
use plan::{
    AgendaRenderKind, ShortcutKind, TaskListMode, plan_agenda_request, plan_task_list_request,
    split_task_list_mode,
};

pub fn run(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    command: &TaskCommand,
) -> Result<ExitCode> {
    let task_id_snapshot = task_id_snapshot_before_command(config, command);
    let exit_code = match command {
        TaskCommand::List(args) => success(run_list(config, ctx, args)),
        TaskCommand::Agenda(args) => success(run_agenda(config, ctx, args)),
        TaskCommand::Inbox(args) => success(run_shortcut(config, ctx, args, ShortcutKind::Inbox)),
        TaskCommand::Show(args) => success(run_show(config, ctx, args)),
        TaskCommand::Open(args) => success(run_open(config, ctx, args)),
        TaskCommand::State(args) => success(run_state(config, ctx, args)),
        TaskCommand::Done(args) => success(run_done(config, ctx, args)),
        TaskCommand::Add(args) => success(run_add(config, ctx, args)),
        TaskCommand::Postpone(args) => success(run_postpone(config, ctx, args)),
        TaskCommand::Target(args) => id_command::run(config, ctx, args),
    }?;
    maybe_warn_task_ids_changed(config, task_id_snapshot);
    Ok(exit_code)
}

fn success(result: Result<()>) -> Result<ExitCode> {
    result.map(|()| ExitCode::SUCCESS)
}

fn run_list(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskListArgs) -> Result<()> {
    let (mode, filters) = split_task_list_mode(&args.filters);
    match mode {
        TaskListMode::Tasks => run_task_list(config, ctx, args, &filters),
        TaskListMode::Projects => run_projects(config, ctx, &filters),
        TaskListMode::Tags => run_tags(config, ctx, &filters),
    }
}

fn run_task_list(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskListArgs,
    raw_filters: &[String],
) -> Result<()> {
    let request = plan_task_list_request(config, args, raw_filters)?;
    if request.uses_pkms_todo_path() {
        let columns = request.pkms_columns().to_vec();
        return todo::run_on(
            config,
            ctx,
            &todo::TodoOptions {
                state: request.filters.criteria.state.clone(),
                tags: request.filters.criteria.tags.clone(),
                kind: request.filters.criteria.kind.clone(),
                sort: request.sort.clone(),
                limit: request.limit,
                group: request.group.clone(),
                scope: request.scope,
                after: request.filters.criteria.after,
                before: request.filters.criteria.before,
                prio: request.filters.criteria.prio.clone(),
                line_sep: request.line_sep,
                columns,
            },
            request.clock,
        );
    }

    let output = execution::execute_task_list(config, &request)?;
    render_task_list(ctx, output)
}

fn render_task_list(ctx: &OutputContext, output: TaskListExecution) -> Result<()> {
    render::print_task_items(
        ctx,
        output.source,
        output.items,
        output.limit,
        output.columns.as_deref(),
    )
}

fn run_shortcut(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<()> {
    let clock = TaskClock::now();
    let mut items = execution::collect_shortcut_items_on(config, &args.filters, kind, clock)?;
    let source = plan::shortcut_display_source(&args.filters, clock.today)?;
    execution::sort_task_items(&mut items, "priority")?;
    let columns = plan::resolve_task_table_columns(
        config,
        source,
        plan::shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn run_agenda(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskAgendaArgs) -> Result<()> {
    let request = plan_agenda_request(config, args)?;
    if request.uses_pkms_agenda_path() {
        let columns = request.pkms_columns().to_vec();
        return agenda::run_with_clock(
            config,
            ctx,
            &agenda::AgendaOptions {
                state: None,
                tags: None,
                kind: None,
                prio: None,
                overdue: false,
                upcoming: false,
                date: None,
                sort: request.sort.clone(),
                limit: request.limit,
                today: false,
                week: false,
                line_sep: request.line_sep,
                columns,
            },
            request.clock,
        );
    }

    let output = execution::execute_task_agenda(config, &request)?;
    render_task_agenda(ctx, output)
}

fn render_task_agenda(ctx: &OutputContext, output: AgendaExecution) -> Result<()> {
    match output.render_kind {
        AgendaRenderKind::TaskItems => render::print_task_items(
            ctx,
            output.source,
            output.items,
            output.limit,
            output.columns.as_deref(),
        ),
        AgendaRenderKind::AgendaGroups => render::print_agenda_task_items(
            ctx,
            output.source,
            output.items,
            output.limit,
            output.columns.as_deref(),
            output.today,
        ),
    }
}

pub(super) fn run_show(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskTargetArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(id) => crate::commands::show::run(
            config,
            ctx,
            &ShowOptions {
                targets: vec![HeadingTarget {
                    note_target: String::new(),
                    canonical_id: Some(id),
                }],
            },
        ),
        TaskId::Todoist(id) => show_todoist_task(config, ctx, &id),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(super) fn run_open(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskOpenArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(id) => crate::commands::open::run(
            config,
            ctx,
            &OpenOptions {
                targets: vec![id.to_string()],
                editor: args.editor.clone(),
                line: args.line,
            },
        ),
        TaskId::Todoist(_) => bail!("Todoist task source is not implemented yet"),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

#[cfg(feature = "todoist")]
fn show_todoist_task(config: &ResolvedConfig, ctx: &OutputContext, id: &str) -> Result<()> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let task = client.get_task(id)?;
    let metadata = if task.project_id.is_some() {
        Some(crate::tasks::todoist::TodoistMetadata::new(
            client.list_projects()?,
        ))
    } else {
        None
    };
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    match ctx.format {
        OutputFormat::Text => render::print_task_table(&[item], 1, SourceSelection::Todoist, None),
        OutputFormat::Json => ctx.print_json(&item),
        OutputFormat::Ndjson => ctx.print_ndjson(&[item]),
    }
}

#[cfg(not(feature = "todoist"))]
fn show_todoist_task(_config: &ResolvedConfig, _ctx: &OutputContext, _id: &str) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

fn run_projects(config: &ResolvedConfig, ctx: &OutputContext, filters: &[String]) -> Result<()> {
    let filters = parse_task_filters(filters)?;
    if filters.todoist_filter.is_some() {
        bail!("Todoist metadata commands do not accept todoist.filter.");
    }
    if filters.has_criteria() {
        bail!("Task metadata commands only accept source filters.");
    }
    let mut rows = providers::collect_task_metadata(
        config,
        filters.source,
        providers::MetadataKind::Projects,
    )?;
    sort_metadata_rows(&mut rows);
    render::print_metadata_rows(ctx, "project", &rows)
}

fn run_tags(config: &ResolvedConfig, ctx: &OutputContext, filters: &[String]) -> Result<()> {
    let filters = parse_task_filters(filters)?;
    if filters.todoist_filter.is_some() {
        bail!("Todoist metadata commands do not accept todoist.filter.");
    }
    if filters.has_criteria() {
        bail!("Task metadata commands only accept source filters.");
    }
    let mut rows =
        providers::collect_task_metadata(config, filters.source, providers::MetadataKind::Tags)?;
    sort_metadata_rows(&mut rows);
    render::print_metadata_rows(ctx, "tag", &rows)
}

fn sort_metadata_rows(rows: &mut [TaskMetadataRow]) {
    rows.sort_by(|a, b| {
        source_sort_key(&a.source)
            .cmp(&source_sort_key(&b.source))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn source_sort_key(source: &TaskSourceKind) -> u8 {
    match source {
        TaskSourceKind::Pkms => 0,
        TaskSourceKind::Todoist => 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskIdSnapshot(Vec<TaskIdentity>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskIdentity {
    path: String,
    ordinal: usize,
}

impl TaskIdSnapshot {
    fn capture(config: &ResolvedConfig) -> Result<Self> {
        let graph = crate::graph::Graph::load(config)?;
        let entries = graph.all_task_entries(config);
        let identities = task_identities_by_location(&entries);
        let mut ordered = Vec::new();
        for (_, path, line_number) in entries {
            let key = (path.clone(), line_number);
            let identity = identities.get(&key).ok_or_else(|| {
                anyhow!("Task ID snapshot missing task identity for {path}:{line_number}")
            })?;
            ordered.push(identity.clone());
        }
        Ok(Self(ordered))
    }
}

fn task_identities_by_location(
    entries: &[(usize, String, usize)],
) -> HashMap<(String, usize), TaskIdentity> {
    let mut line_numbers_by_path: HashMap<&str, Vec<usize>> = HashMap::new();
    for (_, path, line_number) in entries {
        line_numbers_by_path
            .entry(path.as_str())
            .or_default()
            .push(*line_number);
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
    config: &ResolvedConfig,
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

fn maybe_warn_task_ids_changed(config: &ResolvedConfig, before: Option<TaskIdSnapshot>) {
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
        .is_ok_and(|spec| spec.source_or_default().eq_ignore_ascii_case("pkms"))
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
