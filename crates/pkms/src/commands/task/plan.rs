use crate::cli::{
    TaskAgendaArgs, TaskAgendaCommand, TaskListArgs, TaskShortcutArgs, TaskTableArgs,
};
use crate::commands::task_common::{AgendaWindow, RowSeparatorMode};
use crate::config::{ColumnSource, ColumnView, TaskCommandConfig};
use crate::input;
use crate::output::Column;
use crate::util;
use anyhow::{Result, bail};
use pkms_task::{SourceSelection, TaskClock, TaskListView, parse_task_filters_on};

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

#[derive(Debug, Clone)]
pub(super) struct TaskTableOptions {
    pub(super) row_separators: RowSeparatorMode,
    pub(super) columns: Option<Vec<Column>>,
}

pub(super) struct PlannedTaskList {
    pub(super) execution: pkms_task::TaskListRequest,
    pub(super) table: TaskTableOptions,
}

pub(super) struct PlannedAgenda {
    pub(super) execution: pkms_task::AgendaRequest,
    pub(super) table: TaskTableOptions,
}

fn column_source(source: SourceSelection) -> ColumnSource {
    let SourceSelection::Pkms = source;
    ColumnSource::Pkms
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
    config: &TaskCommandConfig,
    args: &TaskListArgs,
    raw_filters: &[String],
    clock: TaskClock,
) -> Result<PlannedTaskList> {
    let filters = parse_task_filters_on(raw_filters, clock.today)?;
    tracing::debug!(
        source = ?filters.source(),
        filter_count = raw_filters.len(),
        has_criteria = filters.has_criteria(),
        "running task list"
    );
    let scope = task_scope(args.from_stdin, filters.scope())?;
    if args.group.is_some() && !matches!(filters.source(), SourceSelection::Pkms) {
        bail!("task list --group is available only for source:pkms");
    }
    if args.from_stdin && !matches!(filters.source(), SourceSelection::Pkms) {
        bail!("task list --from-stdin is available only for source:pkms");
    }

    let columns = resolve_task_table_columns(
        config,
        filters.source(),
        ColumnView::Tasks,
        args.table.columns.as_deref(),
    )?;

    Ok(PlannedTaskList {
        execution: pkms_task::TaskListRequest {
            filters,
            scope,
            sort: args.sort.clone(),
            limit: args.limit,
            group: args.group.clone(),
            clock,
        },
        table: TaskTableOptions {
            row_separators: args.table.line_sep.into(),
            columns,
        },
    })
}

fn task_scope(from_stdin: bool, filter_scope: &[String]) -> Result<Vec<String>> {
    if from_stdin {
        return util::read_stdin_ndjson();
    }
    Ok(filter_scope.to_vec())
}

pub(super) fn resolve_task_table_columns(
    config: &TaskCommandConfig,
    source: SourceSelection,
    view: ColumnView,
    raw_columns: Option<&str>,
) -> Result<Option<Vec<Column>>> {
    if let Some(raw_columns) = raw_columns
        && !input::columns_has_adjustment(raw_columns)
    {
        return input::resolve_columns(Some(raw_columns), None).map(Some);
    }

    let default_columns = config.default_columns(column_source(source), view);
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
    config: &TaskCommandConfig,
    args: &TaskAgendaArgs,
    clock: TaskClock,
) -> Result<PlannedAgenda> {
    match &args.command {
        Some(TaskAgendaCommand::Today(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "today", clock);
        }
        Some(TaskAgendaCommand::Week(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "week", clock);
        }
        Some(TaskAgendaCommand::Overdue(args)) => {
            return plan_agenda_date_shortcut_request(config, args, "overdue", clock);
        }
        Some(TaskAgendaCommand::Upcoming(args)) => {
            let filters = agenda_date_shortcut_filters(&args.filters, "upcoming");
            return plan_agenda_request_from_filters(
                config,
                AgendaPlanInput {
                    raw_filters: &filters,
                    raw_sort: None,
                    limit: args.limit,
                    days: args.days,
                    table: &args.table,
                    clock,
                },
            );
        }
        None => {}
    }

    plan_agenda_request_from_filters(
        config,
        AgendaPlanInput {
            raw_filters: &args.filters,
            raw_sort: args.sort.as_deref(),
            limit: args.limit,
            days: args.days,
            table: &args.table,
            clock,
        },
    )
}

struct AgendaPlanInput<'a> {
    raw_filters: &'a [String],
    raw_sort: Option<&'a str>,
    limit: Option<usize>,
    days: Option<i64>,
    table: &'a TaskTableArgs,
    clock: TaskClock,
}

fn plan_agenda_request_from_filters(
    config: &TaskCommandConfig,
    input: AgendaPlanInput<'_>,
) -> Result<PlannedAgenda> {
    let AgendaPlanInput {
        raw_filters,
        raw_sort,
        limit,
        days,
        table,
        clock,
    } = input;
    let filters = parse_task_filters_on(raw_filters, clock.today)?;
    tracing::debug!(
        source = ?filters.source(),
        filter_count = raw_filters.len(),
        has_criteria = filters.has_criteria(),
        "running task agenda"
    );
    let columns = resolve_task_table_columns(
        config,
        filters.source(),
        ColumnView::Agenda,
        table.columns.as_deref(),
    )?;
    Ok(PlannedAgenda {
        execution: pkms_task::AgendaRequest {
            filters,
            sort: raw_sort.map(str::to_string),
            limit,
            window: AgendaWindow::from_days(days),
            clock,
            view: TaskListView::Agenda,
        },
        table: TaskTableOptions {
            row_separators: table.line_sep.into(),
            columns,
        },
    })
}

fn plan_agenda_date_shortcut_request(
    config: &TaskCommandConfig,
    args: &TaskShortcutArgs,
    date_filter: &str,
    clock: TaskClock,
) -> Result<PlannedAgenda> {
    let filters = agenda_date_shortcut_filters(&args.filters, date_filter);
    plan_agenda_request_from_filters(
        config,
        AgendaPlanInput {
            raw_filters: &filters,
            raw_sort: None,
            limit: args.limit,
            days: None,
            table: &args.table,
            clock,
        },
    )
}

fn agenda_date_shortcut_filters(filters: &[String], date_filter: &str) -> Vec<String> {
    let mut aliased = Vec::with_capacity(filters.len() + 1);
    aliased.push(format!("date:{date_filter}"));
    aliased.extend_from_slice(filters);
    aliased
}
