use crate::cli::{
    TaskAgendaArgs, TaskAgendaCommand, TaskListArgs, TaskShortcutArgs, TaskTableArgs,
};
use crate::config::{ColumnSource, ColumnView, ResolvedConfig};
use crate::input;
use crate::output::Column;
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{SourceSelection, TaskFilters, parse_task_filters_on};
use crate::tasks::provider::TaskListView;
use crate::util;
use anyhow::{Result, bail};
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy)]
pub(super) enum ShortcutKind {
    Inbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskListMode {
    Tasks,
    Projects,
    Tags,
}

pub(super) struct TaskListRequest {
    pub(super) filters: TaskFilters,
    pub(super) scope: Vec<String>,
    pub(super) sort: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) group: Option<String>,
    pub(super) line_sep: bool,
    pub(super) columns: Option<Vec<Column>>,
    pub(super) clock: TaskClock,
}

pub(super) struct AgendaRequest {
    pub(super) filters: TaskFilters,
    pub(super) sort: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) days: Option<i64>,
    pub(super) line_sep: bool,
    pub(super) columns: Option<Vec<Column>>,
    pub(super) clock: TaskClock,
    pub(super) view: TaskListView,
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

pub(super) fn split_task_list_mode(filters: &[String]) -> (TaskListMode, Vec<String>) {
    let Some((first, rest)) = filters.split_first() else {
        return (TaskListMode::Tasks, Vec::new());
    };
    match first.as_str() {
        "projects" => (TaskListMode::Projects, rest.to_vec()),
        "tags" | "labels" => (TaskListMode::Tags, rest.to_vec()),
        _ => (TaskListMode::Tasks, filters.to_vec()),
    }
}

pub(super) fn plan_task_list_request(
    config: &ResolvedConfig,
    args: &TaskListArgs,
    raw_filters: &[String],
) -> Result<TaskListRequest> {
    let clock = TaskClock::now();
    let filters = parse_task_filters_on(raw_filters, clock.today)?;
    tracing::debug!(
        source = ?filters.source,
        filter_count = raw_filters.len(),
        has_todoist_filter = filters.todoist_filter.is_some(),
        has_criteria = filters.has_criteria(),
        "running task list"
    );
    let scope = task_scope(args.from_stdin, &filters.criteria.scope)?;
    if args.group.is_some() && !matches!(filters.source, SourceSelection::Pkms) {
        bail!("task list --group is available only for source:pkms");
    }
    if args.from_stdin && !matches!(filters.source, SourceSelection::Pkms) {
        bail!("task list --from-stdin is available only for source:pkms");
    }

    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Tasks,
        args.table.columns.as_deref(),
    )?;

    Ok(TaskListRequest {
        filters,
        scope,
        sort: args.sort.clone(),
        limit: args.limit,
        group: args.group.clone(),
        line_sep: args.table.line_sep,
        columns,
        clock,
    })
}

fn task_scope(from_stdin: bool, filter_scope: &[String]) -> Result<Vec<String>> {
    if from_stdin {
        return util::read_stdin_ndjson();
    }
    Ok(filter_scope.to_vec())
}

pub(super) fn shortcut_display_source(
    raw_filters: &[String],
    today: NaiveDate,
) -> Result<SourceSelection> {
    let filters = parse_task_filters_on(raw_filters, today)?;
    Ok(filters.source)
}

pub(super) fn resolve_task_table_columns(
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

pub(super) fn shortcut_task_view(kind: ShortcutKind) -> TaskListView {
    match kind {
        ShortcutKind::Inbox => TaskListView::Inbox,
    }
}

pub(super) fn shortcut_column_view(kind: ShortcutKind) -> ColumnView {
    match kind {
        ShortcutKind::Inbox => ColumnView::Tasks,
    }
}

pub(super) fn plan_agenda_request(
    config: &ResolvedConfig,
    args: &TaskAgendaArgs,
) -> Result<AgendaRequest> {
    match &args.command {
        Some(TaskAgendaCommand::Today(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "today");
        }
        Some(TaskAgendaCommand::Week(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "week");
        }
        Some(TaskAgendaCommand::Overdue(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "overdue");
        }
        Some(TaskAgendaCommand::Upcoming(args)) => {
            let filters = agenda_date_shortcut_filters(&args.filters, "upcoming");
            return plan_agenda_request_from_filters(
                config,
                &filters,
                None,
                args.limit,
                args.days,
                &args.table,
            );
        }
        None => {}
    }

    plan_agenda_request_from_filters(
        config,
        &args.filters,
        args.sort.clone(),
        args.limit,
        args.days,
        &args.table,
    )
}

fn plan_agenda_request_from_filters(
    config: &ResolvedConfig,
    raw_filters: &[String],
    sort: Option<String>,
    limit: Option<usize>,
    days: Option<i64>,
    table: &TaskTableArgs,
) -> Result<AgendaRequest> {
    let clock = TaskClock::now();
    let filters = parse_task_filters_on(raw_filters, clock.today)?;
    tracing::debug!(
        source = ?filters.source,
        filter_count = raw_filters.len(),
        has_todoist_filter = filters.todoist_filter.is_some(),
        has_criteria = filters.has_criteria(),
        "running task agenda"
    );
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        ColumnView::Agenda,
        table.columns.as_deref(),
    )?;
    Ok(AgendaRequest {
        filters,
        sort,
        limit,
        days: days.map(|days| days.max(0)),
        line_sep: table.line_sep,
        columns,
        clock,
        view: TaskListView::Agenda,
    })
}

fn plan_agenda_date_shortcut_request(
    config: &ResolvedConfig,
    args: &TaskShortcutArgs,
    date_filter: &str,
) -> Result<AgendaRequest> {
    let filters = agenda_date_shortcut_filters(&args.filters, date_filter);
    plan_agenda_request_from_filters(config, &filters, None, args.limit, None, &args.table)
}

fn agenda_date_shortcut_filters(filters: &[String], date_filter: &str) -> Vec<String> {
    let mut aliased = Vec::with_capacity(filters.len() + 1);
    aliased.push(format!("date:{date_filter}"));
    aliased.extend_from_slice(filters);
    aliased
}
