#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
use crate::cli::{
    TaskAgendaArgs, TaskCommand, TaskListArgs, TaskOpenArgs, TaskShortcutArgs, TaskTargetArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::config::ResolvedConfig;
use crate::output::{Column, OutputContext};
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{
    SourceSelection, TaskFilterContext, TaskFilterCriteria, parse_task_filters,
};
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::provider::{TaskListView, TaskMetadataRow};
use crate::tasks::scope::ResolvedScope;
use crate::workspace::Workspace;
use anyhow::{Result, bail};
use chrono::NaiveDate;

mod agenda;
mod id_command;
mod mutations;
mod plan;
mod providers;
mod render;
mod todo;

use mutations::{
    run_add, run_deadline, run_done, run_postpone, run_schedule, run_state, unsupported_task_source,
};
use plan::{
    AgendaRenderKind, AgendaRequest, ShortcutKind, TaskListMode, TaskListRequest,
    plan_agenda_request, plan_task_list_request, split_task_list_mode,
};

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, command: &TaskCommand) -> Result<()> {
    match command {
        TaskCommand::List(args) => run_list(config, ctx, args),
        TaskCommand::Agenda(args) => run_agenda(config, ctx, args),
        TaskCommand::Inbox(args) => run_shortcut(config, ctx, args, ShortcutKind::Inbox),
        TaskCommand::Show(args) => run_show(config, ctx, args),
        TaskCommand::Open(args) => run_open(config, ctx, args),
        TaskCommand::State(args) => run_state(config, ctx, args),
        TaskCommand::Done(args) => run_done(config, ctx, args),
        TaskCommand::Add(args) => run_add(config, ctx, args),
        TaskCommand::Postpone(args) => run_postpone(config, ctx, args),
        TaskCommand::Schedule(args) => run_schedule(config, ctx, args),
        TaskCommand::Deadline(args) => run_deadline(config, ctx, args),
        TaskCommand::Target(args) => id_command::run(config, ctx, args),
    }
}

struct TaskListExecution {
    source: SourceSelection,
    items: Vec<TaskItem>,
    limit: Option<usize>,
    columns: Option<Vec<Column>>,
}

struct AgendaExecution {
    source: SourceSelection,
    items: Vec<TaskItem>,
    limit: Option<usize>,
    columns: Option<Vec<Column>>,
    today: NaiveDate,
    render_kind: AgendaRenderKind,
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

    let output = execute_task_list(config, &request)?;
    render_task_list(ctx, output)
}

fn execute_task_list(
    config: &ResolvedConfig,
    request: &TaskListRequest,
) -> Result<TaskListExecution> {
    let mut items =
        providers::collect_task_items(config, &request.filters, TaskListView::All, request.clock)?;
    apply_task_filter_criteria_on(
        config,
        &mut items,
        &request.filters.criteria,
        request.clock.today,
    )?;
    sort_task_items(&mut items, request.sort.as_deref().unwrap_or("priority"))?;
    Ok(TaskListExecution {
        source: request.filters.source,
        items,
        limit: request.limit,
        columns: request.source_neutral_columns().map(<[Column]>::to_vec),
    })
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
    let mut items = plan::collect_shortcut_items_on(config, &args.filters, kind, clock)?;
    let source = plan::shortcut_display_source(&args.filters)?;
    sort_task_items(&mut items, "priority")?;
    let columns = plan::resolve_task_table_columns(
        config,
        source,
        plan::shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn apply_task_filter_criteria_on(
    config: &ResolvedConfig,
    items: &mut Vec<TaskItem>,
    criteria: &TaskFilterCriteria,
    today: NaiveDate,
) -> Result<()> {
    let before_count = items.len();
    let scope = if criteria.scope.is_empty() {
        None
    } else {
        let workspace = Workspace::load(config)?;
        Some(ResolvedScope::resolve(
            &workspace.graph,
            config.resolved_db_root(),
            &criteria.scope,
        ))
    };
    let context = TaskFilterContext {
        today,
        scope: scope.as_ref(),
    };
    items.retain(|item| criteria.matches_item(item, &context));
    tracing::debug!(
        before_count,
        after_count = items.len(),
        has_state = criteria.state.is_some(),
        has_tags = criteria.tags.is_some(),
        has_kind = criteria.kind.is_some(),
        has_prio = criteria.prio.is_some(),
        has_date = criteria.date.is_some(),
        has_after = criteria.after.is_some(),
        has_before = criteria.before.is_some(),
        scope_count = criteria.scope.len(),
        has_project = criteria.project.is_some(),
        "applied task filter criteria"
    );

    Ok(())
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

    let output = execute_task_agenda(config, &request)?;
    render_task_agenda(ctx, output)
}

fn execute_task_agenda(
    config: &ResolvedConfig,
    request: &AgendaRequest,
) -> Result<AgendaExecution> {
    let mut items =
        providers::collect_task_items(config, &request.filters, request.view, request.clock)?;
    apply_task_filter_criteria_on(
        config,
        &mut items,
        &request.filters.criteria,
        request.clock.today,
    )?;
    sort_task_items(
        &mut items,
        request.sort.as_deref().unwrap_or("date,priority"),
    )?;
    Ok(AgendaExecution {
        source: request.filters.source,
        items,
        limit: request.limit,
        columns: request.source_neutral_columns().map(<[Column]>::to_vec),
        today: request.clock.today,
        render_kind: request.render_kind,
    })
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

pub(super) fn retain_upcoming_task_items_on(
    items: &mut Vec<TaskItem>,
    days: i64,
    today: NaiveDate,
) {
    let cutoff = today + chrono::Duration::days(days);
    items.retain(|item| {
        item.effective_date()
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .is_some_and(|date| date > today && date <= cutoff)
    });
}

fn sort_task_items(items: &mut [TaskItem], sort: &str) -> Result<()> {
    let fields = parse_task_sort_fields(sort)?;
    items.sort_by(|a, b| {
        for field in &fields {
            let ord = match *field {
                "priority" => a.priority_sort_value().cmp(&b.priority_sort_value()),
                "date" => a.effective_date().cmp(&b.effective_date()),
                "scheduled" => a.scheduled_date_str().cmp(&b.scheduled_date_str()),
                "deadline" => a.deadline_date_str().cmp(&b.deadline_date_str()),
                "file" => a.note_title.cmp(&b.note_title),
                "source" => render::source_name(a).cmp(render::source_name(b)),
                "state" => a.state.cmp(&b.state),
                "task" | "title" => a.title.cmp(&b.title),
                "project" => a.project.cmp(&b.project),
                _ => std::cmp::Ordering::Equal,
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        render::source_name(a)
            .cmp(render::source_name(b))
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(())
}

fn parse_task_sort_fields(sort: &str) -> Result<Vec<&str>> {
    let fields: Vec<&str> = sort
        .split(',')
        .map(|field| field.trim())
        .filter(|field| !field.is_empty())
        .collect();
    if fields.is_empty() {
        bail!("Task sort must include at least one field");
    }
    for field in &fields {
        match *field {
            "priority" | "date" | "scheduled" | "deadline" | "file" | "source" | "state"
            | "task" | "title" | "project" => {}
            other => bail!(
                "Unknown task sort field '{other}'. Use priority, date, scheduled, deadline, file, source, state, task, title, or project."
            ),
        }
    }
    Ok(fields)
}
