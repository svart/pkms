use crate::cli::{
    TaskAgendaArgs, TaskAgendaCommand, TaskListArgs, TaskShortcutArgs, TaskUpcomingArgs,
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
    Today,
    Week,
    Overdue,
    Upcoming { days: i64 },
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
    from_stdin: bool,
    pub(super) line_sep: bool,
    pub(super) columns: TaskListColumns,
    pub(super) clock: TaskClock,
}

pub(super) enum TaskListColumns {
    Pkms(Vec<Column>),
    SourceNeutral(Option<Vec<Column>>),
}

pub(super) struct AgendaRequest {
    pub(super) filters: TaskFilters,
    pub(super) sort: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) days: Option<i64>,
    pub(super) line_sep: bool,
    pub(super) columns: AgendaColumns,
    pub(super) clock: TaskClock,
    pub(super) view: TaskListView,
    pub(super) render_kind: AgendaRenderKind,
}

pub(super) enum AgendaColumns {
    Pkms(Vec<Column>),
    SourceNeutral(Option<Vec<Column>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgendaRenderKind {
    TaskItems,
    AgendaGroups,
}

impl TaskListRequest {
    pub(super) fn uses_pkms_todo_path(&self) -> bool {
        matches!(self.filters.source, SourceSelection::Pkms)
            && (!self.filters.has_criteria() || self.group.is_some() || self.from_stdin)
    }

    pub(super) fn pkms_columns(&self) -> &[Column] {
        match &self.columns {
            TaskListColumns::Pkms(columns) => columns,
            TaskListColumns::SourceNeutral(_) => unreachable!("expected PKMS task columns"),
        }
    }

    pub(super) fn source_neutral_columns(&self) -> Option<&[Column]> {
        match &self.columns {
            TaskListColumns::SourceNeutral(columns) => columns.as_deref(),
            TaskListColumns::Pkms(_) => unreachable!("expected source-neutral task columns"),
        }
    }
}

impl AgendaRequest {
    pub(super) fn uses_pkms_agenda_path(&self) -> bool {
        matches!(&self.columns, AgendaColumns::Pkms(_))
    }

    pub(super) fn pkms_columns(&self) -> &[Column] {
        match &self.columns {
            AgendaColumns::Pkms(columns) => columns,
            AgendaColumns::SourceNeutral(_) => unreachable!("expected PKMS agenda columns"),
        }
    }

    pub(super) fn source_neutral_columns(&self) -> Option<&[Column]> {
        match &self.columns {
            AgendaColumns::SourceNeutral(columns) => columns.as_deref(),
            AgendaColumns::Pkms(_) => unreachable!("expected source-neutral agenda columns"),
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

    let uses_pkms_todo_path = matches!(filters.source, SourceSelection::Pkms)
        && (!filters.has_criteria() || args.group.is_some() || args.from_stdin);
    let columns = if uses_pkms_todo_path {
        TaskListColumns::Pkms(resolve_task_columns(
            config,
            SourceSelection::Pkms,
            ColumnView::Tasks,
            args.table.columns.as_deref(),
        )?)
    } else {
        TaskListColumns::SourceNeutral(resolve_task_table_columns(
            config,
            filters.source,
            ColumnView::Tasks,
            args.table.columns.as_deref(),
        )?)
    };

    Ok(TaskListRequest {
        filters,
        scope,
        sort: args.sort.clone(),
        limit: args.limit,
        group: args.group.clone(),
        from_stdin: args.from_stdin,
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

pub(super) fn shortcut_task_view(kind: ShortcutKind) -> TaskListView {
    match kind {
        ShortcutKind::Today => TaskListView::Today,
        ShortcutKind::Week => TaskListView::Week,
        ShortcutKind::Overdue => TaskListView::Overdue,
        ShortcutKind::Upcoming { days } => TaskListView::Upcoming { days },
        ShortcutKind::Inbox => TaskListView::Inbox,
    }
}

pub(super) fn shortcut_column_view(kind: ShortcutKind) -> ColumnView {
    match kind {
        ShortcutKind::Today
        | ShortcutKind::Week
        | ShortcutKind::Overdue
        | ShortcutKind::Upcoming { .. } => ColumnView::Agenda,
        ShortcutKind::Inbox => ColumnView::Tasks,
    }
}

pub(super) fn plan_agenda_request(
    config: &ResolvedConfig,
    args: &TaskAgendaArgs,
) -> Result<AgendaRequest> {
    match &args.command {
        Some(TaskAgendaCommand::Today(args)) => {
            return plan_agenda_shortcut_request(config, args, ShortcutKind::Today);
        }
        Some(TaskAgendaCommand::Week(args)) => {
            return plan_agenda_shortcut_request(config, args, ShortcutKind::Week);
        }
        Some(TaskAgendaCommand::Overdue(args)) => {
            return plan_agenda_shortcut_request(config, args, ShortcutKind::Overdue);
        }
        Some(TaskAgendaCommand::Upcoming(args)) => {
            return plan_agenda_upcoming_request(config, args);
        }
        None => {}
    }

    let clock = TaskClock::now();
    let filters = parse_task_filters_on(&args.filters, clock.today)?;
    tracing::debug!(
        source = ?filters.source,
        filter_count = args.filters.len(),
        has_todoist_filter = filters.todoist_filter.is_some(),
        has_criteria = filters.has_criteria(),
        "running task agenda"
    );
    let columns = if matches!(filters.source, SourceSelection::Pkms) && !filters.has_criteria() {
        AgendaColumns::Pkms(resolve_task_columns(
            config,
            SourceSelection::Pkms,
            ColumnView::Agenda,
            args.table.columns.as_deref(),
        )?)
    } else {
        AgendaColumns::SourceNeutral(resolve_task_table_columns(
            config,
            filters.source,
            ColumnView::Agenda,
            args.table.columns.as_deref(),
        )?)
    };
    Ok(AgendaRequest {
        filters,
        sort: args.sort.clone(),
        limit: args.limit,
        days: args.days.map(|days| days.max(0)),
        line_sep: args.table.line_sep,
        columns,
        clock,
        view: TaskListView::Agenda,
        render_kind: AgendaRenderKind::AgendaGroups,
    })
}

fn plan_agenda_shortcut_request(
    config: &ResolvedConfig,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<AgendaRequest> {
    let clock = TaskClock::now();
    let filters = parse_task_filters_on(&args.filters, clock.today)?;
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    Ok(AgendaRequest {
        filters,
        sort: Some("priority".to_string()),
        limit: args.limit,
        days: None,
        line_sep: args.table.line_sep,
        columns: AgendaColumns::SourceNeutral(columns),
        clock,
        view: shortcut_task_view(kind),
        render_kind: AgendaRenderKind::TaskItems,
    })
}

fn plan_agenda_upcoming_request(
    config: &ResolvedConfig,
    args: &TaskUpcomingArgs,
) -> Result<AgendaRequest> {
    let clock = TaskClock::now();
    let filters = parse_task_filters_on(&args.filters, clock.today)?;
    let kind = ShortcutKind::Upcoming {
        days: args.days.max(0),
    };
    let columns = resolve_task_table_columns(
        config,
        filters.source,
        shortcut_column_view(kind),
        args.table.columns.as_deref(),
    )?;
    Ok(AgendaRequest {
        filters,
        sort: Some("priority".to_string()),
        limit: args.limit,
        days: None,
        line_sep: args.table.line_sep,
        columns: AgendaColumns::SourceNeutral(columns),
        clock,
        view: shortcut_task_view(kind),
        render_kind: AgendaRenderKind::TaskItems,
    })
}
