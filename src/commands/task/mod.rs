use crate::cli::{
    OutputFormat, TaskAddArgs, TaskAgendaArgs, TaskAgendaCommand, TaskCommand, TaskDeadlineArgs,
    TaskDoneArgs, TaskListArgs, TaskOpenArgs, TaskPostponeArgs, TaskScheduleArgs, TaskShortcutArgs,
    TaskStateArgs, TaskTargetArgs, TaskUpcomingArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::commands::task_common::{RowItem, print_table_with_empty_message};
use crate::commands::task_index::{assign_canonical_ids, collect_todo_records};
use crate::config::{ColumnSource, ColumnView, ResolvedConfig};
use crate::input;
use crate::org_edit;
use crate::output::{ALL_COLUMNS, Column, OutputContext};
use crate::parser::{DEADLINE_RE, HEADING_RE, SCHEDULED_RE, find_daily_file_date};
use crate::tasks::add::{
    TaskAddSpec, org_date, parse_add_date_arg, pkms_priority, validate_pkms_date_arg,
};
use crate::tasks::filter::{
    SourceSelection, TaskFilterContext, TaskFilterCriteria, TaskFilters, parse_task_filters,
};
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms::{self, record_to_task_item};
use crate::tasks::pkms_edit;
use crate::tasks::provider::{
    TaskListView, TaskMetadataRow, TaskProvider, TaskProviderContext, TaskQuery,
};
use crate::tasks::scope::ResolvedScope;
use crate::util;
use crate::workspace::Workspace;
use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tabled::builder::Builder;
use tabled::settings::Style;

mod agenda;
mod todo;

#[cfg(feature = "todoist")]
const PKMS_NOTE_MARKER_PREFIX: &str = "pkms:id:";

#[derive(Debug, Serialize)]
struct TaskStateChangeOutput {
    id: String,
    path: String,
    line_number: usize,
    old_state: String,
    new_state: String,
    dry_run: bool,
}

#[derive(Debug, Clone)]
enum PkmsInboxTarget {
    Note(PathBuf),
    Daily { path: PathBuf },
}

#[derive(Debug, Clone, Copy)]
enum PlanningKind {
    Scheduled,
    Deadline,
}

#[derive(Clone, Copy)]
struct TaskRow<'a> {
    item: &'a TaskItem,
    source: SourceSelection,
}

impl RowItem for TaskRow<'_> {
    fn id(&self) -> usize {
        0
    }

    fn display_id(&self) -> String {
        match self.source {
            SourceSelection::All => match self.item.source {
                TaskSourceKind::Pkms => format!("p{}", self.item.source_id),
                TaskSourceKind::Todoist => format!("t{}", self.item.source_id),
            },
            SourceSelection::Pkms | SourceSelection::Todoist => self.item.source_id.clone(),
        }
    }

    fn todo_state(&self) -> Option<&str> {
        self.item.state.as_deref()
    }

    fn priority(&self) -> Option<char> {
        self.item.priority_char()
    }

    fn title(&self) -> &str {
        self.item.note_title.as_deref().unwrap_or_default()
    }

    fn heading_title(&self) -> &str {
        &self.item.title
    }

    fn project(&self) -> Option<&str> {
        self.item.project.as_deref()
    }

    fn filetags(&self) -> &[String] {
        &self.item.tags
    }

    fn heading_tags(&self) -> &[String] {
        &[]
    }

    fn scheduled(&self) -> Option<&str> {
        self.item.scheduled.as_ref().map(|date| date.raw.as_str())
    }

    fn deadline(&self) -> Option<&str> {
        self.item.deadline.as_ref().map(|date| date.raw.as_str())
    }

    fn daily_file_date(&self) -> Option<&str> {
        self.item.daily_file_date.as_deref()
    }

    fn scheduled_date_str(&self) -> Option<&str> {
        self.item.scheduled_date_str()
    }

    fn deadline_date_str(&self) -> Option<&str> {
        self.item.deadline_date_str()
    }
}

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
        TaskCommand::Target(args) => run_target_command(config, ctx, args),
    }
}

fn run_target_command(config: &ResolvedConfig, ctx: &OutputContext, args: &[String]) -> Result<()> {
    let Some((id, rest)) = args.split_first() else {
        bail!("Expected task ID and subcommand");
    };
    let Some((command, command_args)) = rest.split_first() else {
        bail!("Expected subcommand after task ID. Use: pkms task <ID> <SUBCOMMAND>");
    };

    match command.as_str() {
        "show" => {
            let args = parse_target_show_args(id, command_args)?;
            run_show(config, ctx, &args)
        }
        "open" => {
            let args = parse_target_open_args(id, command_args)?;
            run_open(config, ctx, &args)
        }
        "state" => {
            let args = parse_target_state_args(id, command_args)?;
            run_state(config, ctx, &args)
        }
        "done" => {
            let args = parse_target_done_args(id, command_args)?;
            run_done(config, ctx, &args)
        }
        "postpone" => {
            let args = parse_target_postpone_args(id, command_args)?;
            run_postpone(config, ctx, &args)
        }
        "schedule" => {
            let args = parse_target_schedule_args(id, command_args)?;
            run_schedule(config, ctx, &args)
        }
        "deadline" => {
            let args = parse_target_deadline_args(id, command_args)?;
            run_deadline(config, ctx, &args)
        }
        other => bail!(
            "Unknown task subcommand '{other}' after ID. Expected one of: show, open, state, done, postpone, schedule, deadline"
        ),
    }
}

fn parse_target_show_args(id: &str, raw: &[String]) -> Result<TaskTargetArgs> {
    if let Some(arg) = raw.first() {
        bail!("Unexpected argument for task show: {arg}");
    }
    Ok(TaskTargetArgs { id: id.to_string() })
}

fn parse_target_open_args(id: &str, raw: &[String]) -> Result<TaskOpenArgs> {
    let mut editor = "emacsclient -n".to_string();
    let mut line = None;
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--editor" => {
                editor = iter
                    .next()
                    .cloned()
                    .context("Expected value after --editor")?;
            }
            "--line" | "-l" => {
                let value = iter.next().context("Expected value after --line")?;
                line = Some(
                    value
                        .parse::<usize>()
                        .with_context(|| format!("Invalid line number: {value}"))?,
                );
            }
            other => bail!("Unexpected argument for task open: {other}"),
        }
    }
    Ok(TaskOpenArgs {
        id: id.to_string(),
        editor,
        line,
    })
}

fn parse_target_state_args(id: &str, raw: &[String]) -> Result<TaskStateArgs> {
    let mut state = None;
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other if state.is_none() => state = Some(other.to_string()),
            other => bail!("Unexpected argument for task state: {other}"),
        }
    }
    Ok(TaskStateArgs {
        id: id.to_string(),
        state: state.context("Expected TODO state after task state")?,
        dry_run,
    })
}

fn parse_target_done_args(id: &str, raw: &[String]) -> Result<TaskDoneArgs> {
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other => bail!("Unexpected argument for task done: {other}"),
        }
    }
    Ok(TaskDoneArgs {
        id: id.to_string(),
        dry_run,
    })
}

fn parse_target_postpone_args(id: &str, raw: &[String]) -> Result<TaskPostponeArgs> {
    let to = parse_single_value_option(raw, "--to", "task postpone")?;
    Ok(TaskPostponeArgs {
        id: id.to_string(),
        to,
    })
}

fn parse_target_schedule_args(id: &str, raw: &[String]) -> Result<TaskScheduleArgs> {
    let due = parse_single_value_option(raw, "--due", "task schedule")?;
    Ok(TaskScheduleArgs {
        id: id.to_string(),
        due,
    })
}

fn parse_target_deadline_args(id: &str, raw: &[String]) -> Result<TaskDeadlineArgs> {
    let deadline = parse_single_value_option(raw, "--deadline", "task deadline")?;
    Ok(TaskDeadlineArgs {
        id: id.to_string(),
        deadline,
    })
}

fn parse_single_value_option(raw: &[String], option: &str, command: &str) -> Result<String> {
    let mut value = None;
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        if arg == option {
            if value.is_some() {
                bail!("Option {option} can only be provided once");
            }
            value = Some(
                iter.next()
                    .cloned()
                    .with_context(|| format!("Expected value after {option}"))?,
            );
        } else {
            bail!("Unexpected argument for {command}: {arg}");
        }
    }
    value.with_context(|| format!("Expected {option} for {command}"))
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

struct PkmsTaskProvider<'a> {
    context: TaskProviderContext<'a>,
}

struct TodoistTaskProvider<'a> {
    context: TaskProviderContext<'a>,
}

impl<'a> PkmsTaskProvider<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            context: TaskProviderContext { config },
        }
    }
}

impl<'a> TodoistTaskProvider<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            context: TaskProviderContext { config },
        }
    }
}

struct TaskProviders<'a> {
    pkms: PkmsTaskProvider<'a>,
    todoist: TodoistTaskProvider<'a>,
}

impl<'a> TaskProviders<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            pkms: PkmsTaskProvider::new(config),
            todoist: TodoistTaskProvider::new(config),
        }
    }

    fn list(&self, source: SourceSelection, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        tracing::debug!(
            source = ?source,
            view = ?query.view,
            has_todoist_filter = query.filters.todoist_filter.is_some(),
            "collecting task items from providers"
        );
        match source {
            SourceSelection::Pkms => {
                let items = self.pkms.list(query)?;
                tracing::debug!(
                    source = "pkms",
                    item_count = items.len(),
                    "provider returned tasks"
                );
                Ok(items)
            }
            SourceSelection::Todoist => {
                let items = self.todoist.list(query)?;
                tracing::debug!(
                    source = "todoist",
                    item_count = items.len(),
                    "provider returned tasks"
                );
                Ok(items)
            }
            SourceSelection::All => {
                let mut items = self.pkms.list(query)?;
                let pkms_count = items.len();
                let todoist_items = self.todoist.list(query)?;
                let todoist_count = todoist_items.len();
                items.extend(todoist_items);
                tracing::debug!(
                    pkms_count,
                    todoist_count,
                    total_count = items.len(),
                    "providers returned combined tasks"
                );
                Ok(items)
            }
        }
    }

    fn metadata(
        &self,
        source: SourceSelection,
        kind: MetadataKind,
    ) -> Result<Vec<TaskMetadataRow>> {
        let collect = |provider: &dyn TaskProvider| match kind {
            MetadataKind::Projects => provider.projects(),
            MetadataKind::Tags => provider.tags(),
        };

        match source {
            SourceSelection::Pkms => collect(&self.pkms),
            SourceSelection::Todoist => collect(&self.todoist),
            SourceSelection::All => {
                let mut rows = collect(&self.pkms)?;
                rows.extend(collect(&self.todoist)?);
                Ok(rows)
            }
        }
    }
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

impl TaskProvider for PkmsTaskProvider<'_> {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Pkms
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        match query.view {
            TaskListView::All => pkms::list_items(self.context.config),
            TaskListView::Agenda => pkms::agenda_items(self.context.config),
            TaskListView::Today => {
                pkms::agenda_items_for(self.context.config, true, false, false, false)
            }
            TaskListView::Week => {
                pkms::agenda_items_for(self.context.config, false, true, false, false)
            }
            TaskListView::Overdue => {
                pkms::agenda_items_for(self.context.config, false, false, true, false)
            }
            TaskListView::Upcoming { days } => {
                let mut items =
                    pkms::agenda_items_for(self.context.config, false, false, false, true)?;
                retain_upcoming_task_items(&mut items, days);
                Ok(items)
            }
            TaskListView::Inbox => collect_pkms_inbox_items(self.context.config),
        }
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_project_rows(self.context.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_tag_rows(self.context.config)
    }
}

impl TaskProvider for TodoistTaskProvider<'_> {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Todoist
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        let filters = match query.view {
            TaskListView::All => query.filters.clone(),
            TaskListView::Agenda => query.filters.with_todoist_filter(
                query
                    .filters
                    .todoist_filter
                    .clone()
                    .or_else(|| task_view_todoist_filter(TaskListView::Agenda)),
            ),
            TaskListView::Today
            | TaskListView::Week
            | TaskListView::Overdue
            | TaskListView::Upcoming { .. }
            | TaskListView::Inbox => {
                let shortcut = task_view_todoist_filter(query.view);
                query
                    .filters
                    .with_todoist_filter(query.filters.todoist_filter.clone().or(shortcut))
            }
        };
        collect_todoist_items(self.context.config, &filters)
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_project_rows(self.context.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_label_rows(self.context.config)
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

    let mut items = collect_task_items(config, &filters, TaskListView::All)?;
    apply_task_filter_criteria(config, &mut items, &filters.criteria)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"))?;
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Tasks,
        args.table.columns.as_deref(),
    )?;
    print_task_items(ctx, filters.source, items, args.limit, columns.as_deref())
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

fn collect_task_items(
    config: &ResolvedConfig,
    filters: &TaskFilters,
    view: TaskListView,
) -> Result<Vec<TaskItem>> {
    let query = TaskQuery {
        filters: filters.clone(),
        view,
    };
    let items = TaskProviders::new(config).list(filters.source, &query)?;
    tracing::debug!(
        source = ?filters.source,
        view = ?view,
        item_count = items.len(),
        "collected task items"
    );
    Ok(items)
}

fn run_shortcut(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<()> {
    let mut items = collect_shortcut_items(config, &args.filters, kind)?;
    let source = shortcut_display_source(&args.filters)?;
    sort_task_items(&mut items, "priority")?;
    let columns = resolve_task_table_columns(
        config,
        source,
        shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn run_upcoming(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskUpcomingArgs,
) -> Result<()> {
    let mut items = collect_shortcut_items(
        config,
        &args.filters,
        ShortcutKind::Upcoming {
            days: args.days.max(0),
        },
    )?;
    let source = shortcut_display_source(&args.filters)?;
    sort_task_items(&mut items, "priority")?;
    let columns = resolve_task_table_columns(
        config,
        source,
        ColumnView::Agenda,
        args.table.columns.as_deref(),
    )?;
    print_task_items(ctx, source, items, args.limit, columns.as_deref())
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

fn collect_shortcut_items(
    config: &ResolvedConfig,
    raw_filters: &[String],
    kind: ShortcutKind,
) -> Result<Vec<TaskItem>> {
    let filters = parse_task_filters(raw_filters)?;
    let mut items = collect_task_items(config, &filters, shortcut_task_view(kind))?;
    apply_task_filter_criteria(config, &mut items, &filters.criteria)?;
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

fn apply_task_filter_criteria(
    config: &ResolvedConfig,
    items: &mut Vec<TaskItem>,
    criteria: &TaskFilterCriteria,
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
        today: Local::now().date_naive(),
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

    let mut items = collect_task_items(config, &filters, TaskListView::Agenda)?;
    apply_task_filter_criteria(config, &mut items, &filters.criteria)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("date,priority"))?;
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Agenda,
        args.table.columns.as_deref(),
    )?;
    print_agenda_task_items(ctx, filters.source, items, args.limit, columns.as_deref())
}

fn collect_pkms_inbox_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    let target = resolve_pkms_inbox_target(config, false)?;
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;
    let valid_states = config.todo_states();
    let mut records = collect_todo_records(&workspace.corpus, &valid_states, &[], &[], &[]);
    assign_canonical_ids(config, graph, &mut records);

    let records: Vec<_> = match target {
        PkmsInboxTarget::Note(path) => records
            .into_iter()
            .filter(|record| record.path == path.display().to_string())
            .collect(),
        PkmsInboxTarget::Daily { path } => {
            let section = inbox_section_range(&path)?;
            records
                .into_iter()
                .filter(|record| {
                    record.path == path.display().to_string()
                        && section.is_some_and(|(start, end)| {
                            record.line_number > start && record.line_number < end
                        })
                })
                .collect()
        }
    };

    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
}

fn task_view_todoist_filter(view: TaskListView) -> Option<String> {
    match view {
        TaskListView::All => None,
        TaskListView::Agenda => Some("!no date".to_string()),
        TaskListView::Today => Some("today".to_string()),
        TaskListView::Week => Some("next 7 days".to_string()),
        TaskListView::Overdue => Some("overdue".to_string()),
        TaskListView::Upcoming { days } => Some(format!("due after: today & next {days} days")),
        TaskListView::Inbox => Some("#Inbox".to_string()),
    }
}

fn run_show(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskTargetArgs) -> Result<()> {
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

fn run_open(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskOpenArgs) -> Result<()> {
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
fn collect_todoist_items(config: &ResolvedConfig, filters: &TaskFilters) -> Result<Vec<TaskItem>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let tasks = match filters
        .todoist_filter
        .as_deref()
        .or_else(|| config.todoist_default_filter())
    {
        Some(filter) => client.filter_tasks(filter)?,
        None => client.list_tasks()?,
    };
    let metadata = if tasks.iter().any(|task| task.project_id.is_some()) {
        Some(crate::tasks::todoist::TodoistMetadata::new(
            client.list_projects()?,
        ))
    } else {
        None
    };
    Ok(tasks
        .into_iter()
        .map(|task| crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref()))
        .collect::<Vec<_>>())
    .and_then(|mut items| {
        enrich_todoist_items_with_pkms_notes(config, &mut items)?;
        Ok(items)
    })
}

#[cfg(not(feature = "todoist"))]
fn collect_todoist_items(
    _config: &ResolvedConfig,
    _filters: &TaskFilters,
) -> Result<Vec<TaskItem>> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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
        OutputFormat::Text => print_task_table(&[item], 1, SourceSelection::Todoist, None),
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
    let mut rows = collect_task_metadata(config, filters.source, MetadataKind::Projects)?;
    sort_metadata_rows(&mut rows);
    print_metadata_rows(ctx, "project", &rows)
}

fn run_tags(config: &ResolvedConfig, ctx: &OutputContext, filters: &[String]) -> Result<()> {
    let filters = parse_task_filters(filters)?;
    if filters.todoist_filter.is_some() {
        bail!("Todoist metadata commands do not accept todoist.filter.");
    }
    if filters.has_criteria() {
        bail!("Task metadata commands only accept source filters.");
    }
    let mut rows = collect_task_metadata(config, filters.source, MetadataKind::Tags)?;
    sort_metadata_rows(&mut rows);
    print_metadata_rows(ctx, "tag", &rows)
}

#[derive(Debug, Clone, Copy)]
enum MetadataKind {
    Projects,
    Tags,
}

fn collect_task_metadata(
    config: &ResolvedConfig,
    source: SourceSelection,
    kind: MetadataKind,
) -> Result<Vec<TaskMetadataRow>> {
    TaskProviders::new(config).metadata(source, kind)
}

fn pkms_project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let mut counts = BTreeMap::new();
    for item in pkms::list_items(config)? {
        if let Some(project) = item.project.filter(|project| !project.trim().is_empty()) {
            *counts.entry(project).or_insert(0) += 1;
        }
    }
    Ok(counts
        .into_iter()
        .map(|(project, count)| TaskMetadataRow {
            source: TaskSourceKind::Pkms,
            id: project.clone(),
            name: project,
            count: Some(count),
        })
        .collect())
}

fn pkms_tag_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let mut counts = BTreeMap::new();
    for item in pkms::list_items(config)? {
        for tag in item.tags {
            if !tag.trim().is_empty() {
                *counts.entry(tag).or_insert(0) += 1;
            }
        }
    }
    Ok(counts
        .into_iter()
        .map(|(tag, count)| TaskMetadataRow {
            source: TaskSourceKind::Pkms,
            id: tag.clone(),
            name: tag,
            count: Some(count),
        })
        .collect())
}

#[cfg(feature = "todoist")]
fn todoist_project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    Ok(client
        .list_projects()?
        .into_iter()
        .map(|project| TaskMetadataRow {
            source: TaskSourceKind::Todoist,
            id: project.id,
            name: project.name,
            count: None,
        })
        .collect())
}

#[cfg(not(feature = "todoist"))]
fn todoist_project_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn todoist_label_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    Ok(client
        .list_labels()?
        .into_iter()
        .map(|label| TaskMetadataRow {
            source: TaskSourceKind::Todoist,
            id: label.id,
            name: label.name,
            count: None,
        })
        .collect())
}

#[cfg(not(feature = "todoist"))]
fn todoist_label_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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

fn print_metadata_rows(ctx: &OutputContext, kind: &str, rows: &[TaskMetadataRow]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            if rows.is_empty() {
                println!("No task {kind}s found.");
                return Ok(());
            }
            let mut builder = Builder::new();
            builder.push_record(["Source", "Name", "Count"]);
            for row in rows {
                builder.push_record([
                    format!("{:?}", row.source).to_ascii_lowercase(),
                    row.name.clone(),
                    row.count.map(|count| count.to_string()).unwrap_or_default(),
                ]);
            }
            let mut table = builder.build();
            table.with(Style::blank());
            println!("{table}");
            println!();
            println!("Total: {} task {kind}(s)", rows.len());
            Ok(())
        }
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct MetadataOutput<'a> {
                total: usize,
                items: &'a [TaskMetadataRow],
            }
            ctx.print_json(&MetadataOutput {
                total: rows.len(),
                items: rows,
            })
        }
        OutputFormat::Ndjson => ctx.print_ndjson(rows),
    }
}

fn run_state(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskStateArgs) -> Result<()> {
    match args.id.parse::<TaskId>()? {
        TaskId::Pkms(_) => set_pkms_task_state(config, ctx, &args.id, &args.state, args.dry_run),
        TaskId::Todoist(id) => set_todoist_task_state(config, ctx, &id, &args.state, args.dry_run),
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

fn run_done(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskDoneArgs) -> Result<()> {
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

fn run_postpone(
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

fn run_schedule(
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

fn run_deadline(
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
fn enrich_todoist_items_with_pkms_notes(
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
    let output = replace_heading_state(&path, line_number, &new_state, dry_run)?;
    print_state_change(
        ctx,
        &TaskStateChangeOutput {
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
            print_mutation_output(ctx, "state-open", item)
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

fn resolve_pkms_inbox_target(
    config: &ResolvedConfig,
    create_daily: bool,
) -> Result<PkmsInboxTarget> {
    let target = config.task_inbox()?;
    if target.eq_ignore_ascii_case("daily") {
        return resolve_daily_inbox_target(config, create_daily);
    }

    let graph = crate::graph::Graph::load(config)?;
    if let Some(node) = graph.find_node(target) {
        return Ok(PkmsInboxTarget::Note(node.path.clone()));
    }

    let configured = PathBuf::from(target);
    let candidates = if configured.is_absolute() {
        vec![configured]
    } else {
        vec![config.resolved_db_root().join(&configured), configured]
    };

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(PkmsInboxTarget::Note)
        .ok_or_else(|| anyhow::anyhow!("PKMS task inbox note not found: {target}"))
}

fn resolve_pkms_note_task_target(config: &ResolvedConfig, target: &str) -> Result<PkmsInboxTarget> {
    let graph = crate::graph::Graph::load(config)?;
    if let Some(node) = graph.find_node(target) {
        return Ok(PkmsInboxTarget::Note(node.path.clone()));
    }

    let configured = PathBuf::from(target);
    let candidates = if configured.is_absolute() {
        vec![configured]
    } else {
        vec![config.resolved_db_root().join(&configured), configured]
    };

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(PkmsInboxTarget::Note)
        .ok_or_else(|| anyhow::anyhow!("PKMS task target note not found: {target}"))
}

fn resolve_daily_inbox_target(config: &ResolvedConfig, create: bool) -> Result<PkmsInboxTarget> {
    let today = Local::now().date_naive();
    let configured_path = config
        .resolve_daily_notes_dir()
        .join(format!("{today}.org"));
    if config.daily_notes_dir.is_some() || configured_path.exists() {
        if create {
            ensure_daily_note_exists(&configured_path, today)?;
        }
        return Ok(PkmsInboxTarget::Daily {
            path: configured_path,
        });
    }

    let graph = crate::graph::Graph::load(config)?;
    if let Some(result) = graph
        .results
        .iter()
        .find(|result| find_daily_file_date(&result.path) == Some(today))
    {
        return Ok(PkmsInboxTarget::Daily {
            path: result.path.clone(),
        });
    }

    if !create {
        return Ok(PkmsInboxTarget::Daily {
            path: configured_path,
        });
    }

    ensure_daily_note_exists(&configured_path, today)?;
    Ok(PkmsInboxTarget::Daily {
        path: configured_path,
    })
}

fn ensure_daily_note_exists(path: &Path, today: chrono::NaiveDate) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create daily note directory: {}",
                parent.display()
            )
        })?;
    }
    if !path.exists() {
        let title = today.format("%Y-%m-%d").to_string();
        std::fs::write(path, format!("#+title: {title}\n#+filetags: :daily:\n\n"))
            .with_context(|| format!("Failed to create daily note: {}", path.display()))?;
    }
    Ok(())
}

fn add_pkms_task(config: &ResolvedConfig, ctx: &OutputContext, spec: &TaskAddSpec) -> Result<()> {
    let inbox_target = match spec.note.as_deref() {
        Some(note) => resolve_pkms_note_task_target(config, note)?,
        None => resolve_pkms_inbox_target(config, true)?,
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

    let (inbox_path, line_number) = append_pkms_inbox_entry(&inbox_target, &entry)?;
    let item = find_pkms_task_item(config, &inbox_path, line_number)?.with_context(|| {
        format!(
            "Created task but could not reload it from {}",
            inbox_path.display()
        )
    })?;
    print_add_output(ctx, item)
}

fn append_pkms_inbox_entry(target: &PkmsInboxTarget, entry: &str) -> Result<(PathBuf, usize)> {
    match target {
        PkmsInboxTarget::Note(path) => {
            pkms_edit::append_org_entry(path, entry).map(|line| (path.clone(), line))
        }
        PkmsInboxTarget::Daily { path } => {
            pkms_edit::append_daily_inbox_entry(path, entry).map(|line| (path.clone(), line))
        }
    }
}

fn inbox_section_range(path: &Path) -> Result<Option<(usize, usize)>> {
    pkms_edit::inbox_section_range(path)
}

fn find_pkms_task_item(
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
) -> Result<Option<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;
    let valid_states = config.todo_states();
    let mut records = collect_todo_records(&workspace.corpus, &valid_states, &[], &[], &[]);
    assign_canonical_ids(config, graph, &mut records);
    Ok(records
        .into_iter()
        .find(|record| {
            record.path == path.display().to_string() && record.line_number == line_number
        })
        .map(|record| record_to_task_item(config, record)))
}

fn replace_heading_state(
    path: &str,
    line_number: usize,
    new_state: &str,
    dry_run: bool,
) -> Result<TaskStateChangeOutput> {
    let mut lines = org_edit::read_lines(path)?;
    let idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let line = lines
        .get(idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?
        .clone();
    let (body, newline) = org_edit::split_line_ending(&line);
    let captures = HEADING_RE
        .captures(body)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} is no longer an org heading"))?;
    let state_match = captures
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} does not have a TODO state"))?;
    let old_state = state_match.as_str().to_string();
    if old_state == new_state {
        return Ok(TaskStateChangeOutput {
            id: String::new(),
            path: path.to_string(),
            line_number,
            old_state,
            new_state: new_state.to_string(),
            dry_run,
        });
    }

    let mut updated = body.to_string();
    updated.replace_range(state_match.range(), new_state);
    lines[idx] = format!("{updated}{newline}");
    if !dry_run {
        org_edit::write_lines(path, &lines)?;
    }

    Ok(TaskStateChangeOutput {
        id: String::new(),
        path: path.to_string(),
        line_number,
        old_state,
        new_state: new_state.to_string(),
        dry_run,
    })
}

fn print_state_change(ctx: &OutputContext, output: &TaskStateChangeOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            let action = if output.dry_run {
                "Would change"
            } else {
                "Changed"
            };
            println!(
                "{action} {}:{} from {} to {}",
                output.path, output.line_number, output.old_state, output.new_state
            );
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(output),
        OutputFormat::Ndjson => ctx.print_ndjson(std::slice::from_ref(output)),
    }
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
    update_heading_planning_date(&path, line_number, kind, date.as_deref())?;
    let item = find_pkms_task_item(config, Path::new(&path), line_number)?.with_context(|| {
        format!("Changed task but could not reload it from {path}:{line_number}")
    })?;
    print_mutation_output(
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
    update_recurring_planning_date(&path, line_number, &date)?;
    let item = find_pkms_task_item(config, Path::new(&path), line_number)?.with_context(|| {
        format!("Changed task but could not reload it from {path}:{line_number}")
    })?;
    print_mutation_output(ctx, "postpone", item)
}

fn update_heading_planning_date(
    path: &str,
    line_number: usize,
    kind: PlanningKind,
    date: Option<&str>,
) -> Result<()> {
    let mut lines = org_edit::read_lines(path)?;
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let heading = lines
        .get(heading_idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?;
    if !HEADING_RE.is_match(heading.trim_end()) {
        bail!("Task line {line_number} is no longer an org heading");
    }

    let planning_idx = find_planning_line_index(&lines, heading_idx);
    match (planning_idx, date) {
        (Some(idx), Some(date)) => {
            let updated = replace_planning_token(&lines[idx], kind, Some(&org_date(date)?));
            lines[idx] = updated;
        }
        (Some(idx), None) => {
            let updated = replace_planning_token(&lines[idx], kind, None);
            if updated.trim().is_empty() {
                lines.remove(idx);
            } else {
                lines[idx] = updated;
            }
        }
        (None, Some(date)) => {
            lines.insert(
                heading_idx + 1,
                format!("{}: {}\n", planning_label(kind), org_date(date)?),
            );
        }
        (None, None) => {}
    }
    org_edit::write_lines(path, &lines)?;
    Ok(())
}

fn find_planning_line_index(lines: &[String], heading_idx: usize) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(heading_idx + 1) {
        let body = line.trim_end();
        if HEADING_RE.is_match(body) {
            return None;
        }
        if body.trim().is_empty() {
            continue;
        }
        if SCHEDULED_RE.is_match(body) || DEADLINE_RE.is_match(body) {
            return Some(idx);
        }
        return None;
    }
    None
}

fn update_recurring_planning_date(path: &str, line_number: usize, date: &str) -> Result<()> {
    let new_date = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    let mut lines = org_edit::read_lines(path)?;
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let planning_idx = find_planning_line_index(&lines, heading_idx).ok_or_else(|| {
        anyhow::anyhow!("Task does not have a recurring scheduled or deadline date")
    })?;
    lines[planning_idx] = postpone_recurring_planning_line(&lines[planning_idx], new_date)?;
    org_edit::write_lines(path, &lines)?;
    Ok(())
}

fn postpone_recurring_planning_line(line: &str, new_date: NaiveDate) -> Result<String> {
    if let Some(updated) = postpone_recurring_token(line, &SCHEDULED_RE, "SCHEDULED", new_date)? {
        return Ok(updated);
    }
    if let Some(updated) = postpone_recurring_token(line, &DEADLINE_RE, "DEADLINE", new_date)? {
        return Ok(updated);
    }
    bail!("Task does not have a recurring scheduled or deadline date")
}

fn postpone_recurring_token(
    line: &str,
    regex: &regex::Regex,
    label: &str,
    new_date: NaiveDate,
) -> Result<Option<String>> {
    let Some(captures) = regex.captures(line) else {
        return Ok(None);
    };
    let Some(raw_match) = captures.get(1) else {
        return Ok(None);
    };
    let parsed = crate::org_date::parse_org_date(raw_match.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not parse existing {label} date"))?;
    if parsed.repeater.is_none() {
        bail!("Task {label} date is not recurring");
    }
    let replacement = format!("{label}: {}", format_org_date_like(&parsed, new_date));
    Ok(Some(regex.replace(line, replacement.as_str()).to_string()))
}

fn format_org_date_like(existing: &crate::org_date::OrgDate, new_date: NaiveDate) -> String {
    let open = if existing.inactive { "[" } else { "<" };
    let close = if existing.inactive { "]" } else { ">" };
    let mut parts = vec![new_date.format("%Y-%m-%d %a").to_string()];
    if let Some(time) = existing.time {
        let mut time_part = time.format("%H:%M").to_string();
        if let Some(end) = existing.time_end {
            time_part.push('-');
            time_part.push_str(&end.format("%H:%M").to_string());
        }
        parts.push(time_part);
    }
    if let Some(repeater) = existing.repeater.as_deref() {
        parts.push(repeater.to_string());
    }
    if let Some(warning) = existing.warning.as_deref() {
        parts.push(warning.to_string());
    }
    format!("{open}{}{close}", parts.join(" "))
}

fn planning_label(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Scheduled => "SCHEDULED",
        PlanningKind::Deadline => "DEADLINE",
    }
}

fn replace_planning_token(line: &str, kind: PlanningKind, value: Option<&str>) -> String {
    let (body, newline) = org_edit::split_line_ending(line);
    let regex = match kind {
        PlanningKind::Scheduled => &*SCHEDULED_RE,
        PlanningKind::Deadline => &*DEADLINE_RE,
    };
    let label = planning_label(kind);
    let updated = if regex.is_match(body) {
        match value {
            Some(value) => regex
                .replace(body, format!("{label}: {value}").as_str())
                .to_string(),
            None => regex.replace(body, "").to_string(),
        }
    } else {
        match value {
            Some(value) if body.trim().is_empty() => format!("{label}: {value}"),
            Some(value) => format!("{} {label}: {value}", body.trim_end()),
            None => body.to_string(),
        }
    };
    format!("{}{}", updated.trim(), newline)
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
    print_add_output(ctx, item)
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
    print_add_output(ctx, item)
}

fn print_add_output(ctx: &OutputContext, item: TaskItem) -> Result<()> {
    #[derive(Serialize)]
    struct AddOutput {
        created: bool,
        item: TaskItem,
    }

    let output = AddOutput {
        created: true,
        item,
    };
    match ctx.format {
        OutputFormat::Text => {
            print_created_task(&output.item);
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(&output),
        OutputFormat::Ndjson => ctx.print_ndjson(&[output]),
    }
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
    print_mutation_output(ctx, action, item)
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
    print_mutation_output(ctx, "postpone", item)
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

fn print_mutation_output(ctx: &OutputContext, action: &'static str, item: TaskItem) -> Result<()> {
    #[derive(Serialize)]
    struct MutationOutput {
        changed: bool,
        action: &'static str,
        item: TaskItem,
    }

    let output = MutationOutput {
        changed: true,
        action,
        item,
    };
    match ctx.format {
        OutputFormat::Text => {
            println!(
                "Changed {} task: {} (action {}; id {})",
                source_name(&output.item),
                output.item.title,
                action,
                output.item.display_id
            );
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(&output),
        OutputFormat::Ndjson => ctx.print_ndjson(&[output]),
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

fn print_created_task(item: &TaskItem) {
    let mut details = vec![format!("id {}", item.display_id)];
    if let Some(date) = item.effective_date() {
        details.push(format!("date {date}"));
    }
    if let Some(priority) = item.priority.as_deref() {
        details.push(format!("priority {priority}"));
    }
    if let Some(project) = item.project.as_deref() {
        details.push(format!("project {project}"));
    }
    if !item.tags.is_empty() {
        details.push(format!("labels {}", item.tags.join(", ")));
    }
    println!(
        "Created {} task: {} ({})",
        source_display_name(item),
        item.title,
        details.join("; ")
    );
}

fn source_display_name(item: &TaskItem) -> &'static str {
    match item.source {
        TaskSourceKind::Pkms => "PKMS",
        TaskSourceKind::Todoist => "Todoist",
    }
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

fn retain_upcoming_task_items(items: &mut Vec<TaskItem>, days: i64) {
    let today = Local::now().date_naive();
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
                "source" => source_name(a).cmp(source_name(b)),
                "state" => a.state.cmp(&b.state),
                "task" | "title" => a.title.cmp(&b.title),
                "project" => a.project.cmp(&b.project),
                _ => std::cmp::Ordering::Equal,
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        source_name(a)
            .cmp(source_name(b))
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

fn print_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
    columns: Option<&[Column]>,
) -> Result<()> {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }

    match ctx.format {
        OutputFormat::Text => print_task_table(&items, total, source, columns),
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct TaskListOutput {
                total: usize,
                items: Vec<TaskItem>,
            }
            ctx.print_json(&TaskListOutput { total, items })
        }
        OutputFormat::Ndjson => ctx.print_ndjson(&items),
    }
}

fn print_agenda_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
    columns: Option<&[Column]>,
) -> Result<()> {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }

    match ctx.format {
        OutputFormat::Text => print_agenda_task_table(&items, total, source, columns),
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct TaskListOutput {
                total: usize,
                items: Vec<TaskItem>,
            }
            ctx.print_json(&TaskListOutput { total, items })
        }
        OutputFormat::Ndjson => ctx.print_ndjson(&items),
    }
}

fn print_task_table(
    items: &[TaskItem],
    total: usize,
    source: SourceSelection,
    columns: Option<&[Column]>,
) -> Result<()> {
    let rows = task_rows(items, source);
    let sections = [("", rows.as_slice())];
    let footer = format!("Shown: {}, Total: {} task(s)", rows.len(), total);
    print_table_with_empty_message(
        &sections,
        task_columns(columns),
        false,
        &footer,
        "No tasks found.",
    );
    Ok(())
}

fn print_agenda_task_table(
    items: &[TaskItem],
    total: usize,
    source: SourceSelection,
    columns: Option<&[Column]>,
) -> Result<()> {
    let today = Local::now().date_naive();
    let mut overdue = Vec::new();
    let mut today_items = Vec::new();
    let mut upcoming = Vec::new();

    for item in items {
        if item.is_overdue_on(today) {
            overdue.push(item.clone());
        } else if item.is_today_on(today) {
            today_items.push(item.clone());
        } else if item.effective_date().is_some() {
            upcoming.push(item.clone());
        }
    }

    overdue.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
    today_items.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
    upcoming.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));

    let overdue_rows = task_rows(&overdue, source);
    let today_rows = task_rows(&today_items, source);
    let upcoming_rows = task_rows(&upcoming, source);
    let sections = [
        ("=== Overdue ===", overdue_rows.as_slice()),
        ("=== Today ===", today_rows.as_slice()),
        ("=== Upcoming ===", upcoming_rows.as_slice()),
    ];
    let item_count = overdue_rows.len() + today_rows.len() + upcoming_rows.len();
    let footer = format!("Shown: {}, Total: {} task(s)", item_count, total);
    print_table_with_empty_message(
        &sections,
        task_columns(columns),
        false,
        &footer,
        "No tasks found.",
    );
    Ok(())
}

fn task_rows(items: &[TaskItem], source: SourceSelection) -> Vec<TaskRow<'_>> {
    items.iter().map(|item| TaskRow { item, source }).collect()
}

fn task_columns(columns: Option<&[Column]>) -> &[Column] {
    columns.unwrap_or(ALL_COLUMNS.as_slice())
}

fn source_name(item: &TaskItem) -> &'static str {
    match item.source {
        crate::tasks::model::TaskSourceKind::Pkms => "pkms",
        crate::tasks::model::TaskSourceKind::Todoist => "todoist",
    }
}

fn unsupported_task_source(source: &str) -> Result<()> {
    bail!("Task source '{source}' is not configured in this build.")
}
