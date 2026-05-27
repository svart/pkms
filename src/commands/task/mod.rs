#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
use crate::cli::{
    TaskAddArgs, TaskAgendaArgs, TaskAgendaCommand, TaskCommand, TaskDeadlineArgs, TaskDoneArgs,
    TaskListArgs, TaskOpenArgs, TaskPostponeArgs, TaskScheduleArgs, TaskShortcutArgs,
    TaskStateArgs, TaskTargetArgs, TaskUpcomingArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::config::{ColumnSource, ColumnView, ResolvedConfig};
use crate::input;
use crate::output::{Column, OutputContext};
use crate::tasks::add::{
    TaskAddSpec, org_date, parse_add_date_arg, pkms_priority, validate_pkms_date_arg,
};
use crate::tasks::filter::{
    SourceSelection, TaskFilterContext, TaskFilterCriteria, parse_task_filters,
};
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms::{self, PkmsInboxTarget};
use crate::tasks::pkms_mutation::{self, PlanningKind};
use crate::tasks::provider::{TaskListView, TaskMetadataRow};
use crate::tasks::scope::ResolvedScope;
use crate::util;
use crate::workspace::Workspace;
use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
#[cfg(feature = "todoist")]
use serde::Serialize;
use std::path::Path;

mod agenda;
mod id_command;
mod providers;
mod render;
mod todo;

#[cfg(feature = "todoist")]
const PKMS_NOTE_MARKER_PREFIX: &str = "pkms:id:";

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

#[derive(Debug, Clone, Copy)]
enum ShortcutKind {
    Today,
    Week,
    Overdue,
    Upcoming { days: i64 },
    Inbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskListMode {
    Tasks,
    Projects,
    Tags,
}

impl SourceSelection {
    fn column_source(self) -> ColumnSource {
        match self {
            SourceSelection::Pkms => ColumnSource::Pkms,
            SourceSelection::Todoist => ColumnSource::Todoist,
            SourceSelection::All => ColumnSource::All,
        }
    }
}

fn run_list(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskListArgs) -> Result<()> {
    let (mode, filters) = split_task_list_mode(&args.filters);
    match mode {
        TaskListMode::Tasks => run_task_list(config, ctx, args, &filters),
        TaskListMode::Projects => run_projects(config, ctx, &filters),
        TaskListMode::Tags => run_tags(config, ctx, &filters),
    }
}

fn split_task_list_mode(filters: &[String]) -> (TaskListMode, Vec<String>) {
    let Some((first, rest)) = filters.split_first() else {
        return (TaskListMode::Tasks, Vec::new());
    };
    match first.as_str() {
        "projects" => (TaskListMode::Projects, rest.to_vec()),
        "tags" | "labels" => (TaskListMode::Tags, rest.to_vec()),
        _ => (TaskListMode::Tasks, filters.to_vec()),
    }
}

fn run_task_list(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskListArgs,
    raw_filters: &[String],
) -> Result<()> {
    let filters = parse_task_filters(raw_filters)?;
    tracing::debug!(
        source = ?filters.source,
        filter_count = raw_filters.len(),
        has_todoist_filter = filters.todoist_filter.is_some(),
        has_criteria = filters.has_criteria(),
        "running task list"
    );
    let scope = task_scope(config, args.from_stdin, &filters.criteria.scope)?;
    if args.group.is_some() && !matches!(filters.source, SourceSelection::Pkms) {
        bail!("task list --group is available only for source:pkms");
    }
    if args.from_stdin && !matches!(filters.source, SourceSelection::Pkms) {
        bail!("task list --from-stdin is available only for source:pkms");
    }
    if matches!(filters.source, SourceSelection::Pkms)
        && (!filters.has_criteria() || args.group.is_some() || args.from_stdin)
    {
        let columns = resolve_task_columns(
            config,
            SourceSelection::Pkms,
            ColumnView::Tasks,
            args.table.columns.as_deref(),
        )?;
        return todo::run(
            config,
            ctx,
            &todo::TodoOptions {
                state: filters.criteria.state.clone(),
                tags: filters.criteria.tags.clone(),
                kind: filters.criteria.kind.clone(),
                sort: args.sort.clone(),
                limit: args.limit,
                group: args.group.clone(),
                scope,
                after: filters.criteria.after,
                before: filters.criteria.before,
                prio: filters.criteria.prio.clone(),
                line_sep: args.table.line_sep,
                columns,
            },
        );
    }

    let today = Local::now().date_naive();
    let mut items = providers::collect_task_items(config, &filters, TaskListView::All, today)?;
    apply_task_filter_criteria_on(config, &mut items, &filters.criteria, today)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"))?;
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Tasks,
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(ctx, filters.source, items, args.limit, columns.as_deref())
}

fn task_scope(
    _config: &ResolvedConfig,
    from_stdin: bool,
    filter_scope: &[String],
) -> Result<Vec<String>> {
    if from_stdin {
        return util::read_stdin_ndjson();
    }
    Ok(filter_scope.to_vec())
}

fn run_shortcut(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<()> {
    let today = Local::now().date_naive();
    let mut items = collect_shortcut_items_on(config, &args.filters, kind, today)?;
    let source = shortcut_display_source(&args.filters)?;
    sort_task_items(&mut items, "priority")?;
    let columns = resolve_task_table_columns(
        config,
        source,
        shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn run_upcoming(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskUpcomingArgs,
) -> Result<()> {
    let today = Local::now().date_naive();
    let mut items = collect_shortcut_items_on(
        config,
        &args.filters,
        ShortcutKind::Upcoming {
            days: args.days.max(0),
        },
        today,
    )?;
    let source = shortcut_display_source(&args.filters)?;
    sort_task_items(&mut items, "priority")?;
    let columns = resolve_task_table_columns(
        config,
        source,
        ColumnView::Agenda,
        args.table.columns.as_deref(),
    )?;
    render::print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn shortcut_display_source(raw_filters: &[String]) -> Result<SourceSelection> {
    let filters = parse_task_filters(raw_filters)?;
    Ok(filters.source)
}

fn resolve_task_table_columns(
    config: &ResolvedConfig,
    source: SourceSelection,
    view: ColumnView,
    raw_columns: Option<&str>,
) -> Result<Option<Vec<Column>>> {
    if let Some(raw_columns) = raw_columns
        && !input::columns_has_adjustment(raw_columns)
    {
        return input::resolve_columns(Some(raw_columns), None).map(Some);
    }

    let default_columns = config.default_columns(source.column_source(), view)?;
    if raw_columns.is_none() && default_columns.is_none() {
        return Ok(None);
    }
    input::resolve_columns(raw_columns, default_columns).map(Some)
}

fn resolve_task_columns(
    config: &ResolvedConfig,
    source: SourceSelection,
    view: ColumnView,
    raw_columns: Option<&str>,
) -> Result<Vec<Column>> {
    input::resolve_columns(
        raw_columns,
        config.default_columns(source.column_source(), view)?,
    )
}

fn collect_shortcut_items_on(
    config: &ResolvedConfig,
    raw_filters: &[String],
    kind: ShortcutKind,
    today: NaiveDate,
) -> Result<Vec<TaskItem>> {
    let filters = parse_task_filters(raw_filters)?;
    let mut items =
        providers::collect_task_items(config, &filters, shortcut_task_view(kind), today)?;
    apply_task_filter_criteria_on(config, &mut items, &filters.criteria, today)?;
    Ok(items)
}

fn shortcut_task_view(kind: ShortcutKind) -> TaskListView {
    match kind {
        ShortcutKind::Today => TaskListView::Today,
        ShortcutKind::Week => TaskListView::Week,
        ShortcutKind::Overdue => TaskListView::Overdue,
        ShortcutKind::Upcoming { days } => TaskListView::Upcoming { days },
        ShortcutKind::Inbox => TaskListView::Inbox,
    }
}

fn shortcut_column_view(kind: ShortcutKind) -> ColumnView {
    match kind {
        ShortcutKind::Today
        | ShortcutKind::Week
        | ShortcutKind::Overdue
        | ShortcutKind::Upcoming { .. } => ColumnView::Agenda,
        ShortcutKind::Inbox => ColumnView::Tasks,
    }
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
    match &args.command {
        Some(TaskAgendaCommand::Today(args)) => {
            return run_shortcut(config, ctx, args, ShortcutKind::Today);
        }
        Some(TaskAgendaCommand::Week(args)) => {
            return run_shortcut(config, ctx, args, ShortcutKind::Week);
        }
        Some(TaskAgendaCommand::Overdue(args)) => {
            return run_shortcut(config, ctx, args, ShortcutKind::Overdue);
        }
        Some(TaskAgendaCommand::Upcoming(args)) => return run_upcoming(config, ctx, args),
        None => {}
    }

    let filters = parse_task_filters(&args.filters)?;
    tracing::debug!(
        source = ?filters.source,
        filter_count = args.filters.len(),
        has_todoist_filter = filters.todoist_filter.is_some(),
        has_criteria = filters.has_criteria(),
        "running task agenda"
    );
    if matches!(filters.source, SourceSelection::Pkms) && !filters.has_criteria() {
        let columns = resolve_task_columns(
            config,
            SourceSelection::Pkms,
            ColumnView::Agenda,
            args.table.columns.as_deref(),
        )?;
        return agenda::run(
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
                sort: args.sort.clone(),
                limit: args.limit,
                today: false,
                week: false,
                line_sep: args.table.line_sep,
                columns,
            },
        );
    }

    let today = Local::now().date_naive();
    let mut items = providers::collect_task_items(config, &filters, TaskListView::Agenda, today)?;
    apply_task_filter_criteria_on(config, &mut items, &filters.criteria, today)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("date,priority"))?;
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Agenda,
        args.table.columns.as_deref(),
    )?;
    render::print_agenda_task_items(ctx, filters.source, items, args.limit, columns.as_deref())
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
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
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

pub(super) fn run_state(
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

pub(super) fn run_done(
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

fn run_add(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskAddArgs) -> Result<()> {
    let spec = TaskAddSpec::parse(&args.text)?;
    if spec.source.eq_ignore_ascii_case("pkms") {
        return add_pkms_task(config, ctx, &spec);
    }
    if spec.source.eq_ignore_ascii_case("todoist") {
        return add_todoist_task(config, ctx, &spec);
    }
    bail!(
        "Unknown task source '{}'. Use pkms or todoist.",
        spec.source
    )
}

pub(super) fn run_postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskPostponeArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => {
            postpone_pkms_recurring_task(config, ctx, canonical_id, &args.to)
        }
        TaskId::Todoist(id) => postpone_todoist_recurring_task(config, ctx, &id, &args.to),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(super) fn run_schedule(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskScheduleArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => set_pkms_task_planning(
            config,
            ctx,
            canonical_id,
            PlanningKind::Scheduled,
            &args.due,
            "schedule",
            "unschedule",
        ),
        TaskId::Todoist(id) => {
            let due = mutation_date_value(&args.due)?;
            mutate_todoist_task(
                config,
                ctx,
                &id,
                if due.is_null() {
                    "unschedule"
                } else {
                    "schedule"
                },
                serde_json::json!({ "due_date": due }),
            )
        }
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(super) fn run_deadline(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskDeadlineArgs,
) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => set_pkms_task_planning(
            config,
            ctx,
            canonical_id,
            PlanningKind::Deadline,
            &args.deadline,
            "deadline",
            "clear-deadline",
        ),
        TaskId::Todoist(id) => {
            let deadline = mutation_date_value(&args.deadline)?;
            mutate_todoist_task(
                config,
                ctx,
                &id,
                if deadline.is_null() {
                    "clear-deadline"
                } else {
                    "deadline"
                },
                serde_json::json!({ "deadline_date": deadline }),
            )
        }
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

#[cfg(feature = "todoist")]
fn pkms_note_marker_uuid(description: &str) -> Option<String> {
    description.split_whitespace().find_map(|part| {
        part.strip_prefix(PKMS_NOTE_MARKER_PREFIX)
            .filter(|uuid| !uuid.is_empty())
            .map(str::to_string)
    })
}

#[cfg(feature = "todoist")]
pub(super) fn enrich_todoist_items_with_pkms_notes(
    config: &ResolvedConfig,
    items: &mut [TaskItem],
) -> Result<()> {
    if !items.iter().any(|item| {
        item.body
            .as_deref()
            .and_then(pkms_note_marker_uuid)
            .is_some()
    }) {
        return Ok(());
    }

    let graph = crate::graph::Graph::load(config)?;
    for item in items {
        let Some(uuid) = item.body.as_deref().and_then(pkms_note_marker_uuid) else {
            continue;
        };
        item.note_uuid = Some(uuid.clone());
        if let Some(node) = graph.find_node(&uuid) {
            item.note_title = Some(node.title.clone());
        }
    }
    Ok(())
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
            enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
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

fn add_pkms_task(config: &ResolvedConfig, ctx: &OutputContext, spec: &TaskAddSpec) -> Result<()> {
    let inbox_target = match spec.note.as_deref() {
        Some(note) => pkms::resolve_note_task_target(config, note)?,
        None => pkms::resolve_inbox_target(config, true)?,
    };
    let title = spec
        .title
        .as_deref()
        .or(spec.text.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| anyhow::anyhow!("PKMS task creation requires task text or title:"))?;

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

    let level = match inbox_target {
        PkmsInboxTarget::Note(_) => "*",
        PkmsInboxTarget::Daily { .. } => "**",
    };
    let mut entry = format!("{level} {state}{priority} {title}{tags}\n");
    let due = validate_pkms_date_arg("due", spec.due.as_deref())?;
    let deadline = validate_pkms_date_arg("deadline", spec.deadline.as_deref())?;
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

    let (inbox_path, line_number) = pkms::append_inbox_entry(&inbox_target, &entry)?;
    let item = pkms::find_task_item(config, &inbox_path, line_number)?.with_context(|| {
        format!(
            "Created task but could not reload it from {}",
            inbox_path.display()
        )
    })?;
    render::print_add_output(ctx, item)
}

fn set_pkms_task_planning(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    kind: PlanningKind,
    value: &str,
    set_action: &'static str,
    clear_action: &'static str,
) -> Result<()> {
    let date = if value.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(parse_mutation_due_date(value)?)
    };
    let graph = crate::graph::Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    pkms_mutation::update_heading_planning_date(&path, line_number, kind, date.as_deref())?;
    let item = pkms::find_task_item(config, Path::new(&path), line_number)?.with_context(|| {
        format!("Changed task but could not reload it from {path}:{line_number}")
    })?;
    render::print_mutation_output(
        ctx,
        if date.is_some() {
            set_action
        } else {
            clear_action
        },
        item,
    )
}

fn postpone_pkms_recurring_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    to: &str,
) -> Result<()> {
    let date = parse_mutation_due_date(to)?;
    let graph = crate::graph::Graph::load(config)?;
    let (path, line_number) = graph.resolve_canonical_task_id(config, canonical_id)?;
    pkms_mutation::update_recurring_planning_date(&path, line_number, &date)?;
    let item = pkms::find_task_item(config, Path::new(&path), line_number)?.with_context(|| {
        format!("Changed task but could not reload it from {path}:{line_number}")
    })?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(feature = "todoist")]
fn add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskAddSpec,
) -> Result<()> {
    if spec.note.is_some() {
        bail!("note is available only for PKMS task creation.");
    }
    if is_structured_add(spec) {
        return create_structured_todoist_task(config, ctx, spec);
    }
    quick_add_todoist_task(config, ctx, spec)
}

#[cfg(not(feature = "todoist"))]
fn add_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _spec: &TaskAddSpec,
) -> Result<()> {
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
    let due_date = validate_date_arg("due", spec.due.as_deref())?;
    let deadline_date = validate_date_arg("deadline", spec.deadline.as_deref())?;
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
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
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
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
fn mutate_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    action: &'static str,
    request: serde_json::Value,
) -> Result<()> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    client.update_task(id, &request)?;
    let task = client.get_task(id)?;
    let metadata = todoist_metadata_for_task(&client, &task)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_mutation_output(ctx, action, item)
}

#[cfg(feature = "todoist")]
fn postpone_todoist_recurring_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    to: &str,
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
        &serde_json::json!({ "due_date": parse_mutation_due_date(to)? }),
    )?;
    let task = client.get_task(id)?;
    let metadata = todoist_metadata_for_task(&client, &task)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref());
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(not(feature = "todoist"))]
fn postpone_todoist_recurring_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _to: &str,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(not(feature = "todoist"))]
fn mutate_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _action: &'static str,
    _request: serde_json::Value,
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
fn validate_date_arg(name: &str, value: Option<&str>) -> Result<Option<String>> {
    value
        .map(|value| parse_add_date_arg(name, value))
        .transpose()
}

fn parse_mutation_due_date(value: &str) -> Result<String> {
    parse_add_date_arg("due", value)
}

fn mutation_date_value(value: &str) -> Result<serde_json::Value> {
    if value.eq_ignore_ascii_case("none") {
        Ok(serde_json::Value::Null)
    } else {
        Ok(serde_json::Value::String(parse_mutation_due_date(value)?))
    }
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

fn unsupported_task_source(source: &str) -> Result<()> {
    bail!("Task source '{source}' is not configured in this build.")
}
