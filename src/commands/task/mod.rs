use crate::cli::{
    OutputFormat, TaskAddArgs, TaskAgendaArgs, TaskAgendaCommand, TaskCommand, TaskDeadlineArgs,
    TaskDoneArgs, TaskListArgs, TaskOpenArgs, TaskPostponeArgs, TaskScheduleArgs, TaskShortcutArgs,
    TaskStateArgs, TaskTargetArgs, TaskUpcomingArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::commands::task_index::{
    assign_canonical_ids, collect_agenda_records, collect_todo_records,
};
use crate::config::ResolvedConfig;
use crate::input;
use crate::org_date::parse_org_date;
use crate::output::{Column, OutputContext};
use crate::parser::{DEADLINE_RE, HEADING_RE, SCHEDULED_RE, find_daily_file_date};
use crate::tasks::filter::{
    SourceSelection, TaskDateFilter, TaskFilterCriteria, TaskFilters, parse_task_filters,
};
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms::record_to_task_item;
use crate::tasks::provider::{
    TaskListView, TaskMetadataRow, TaskProvider, TaskProviderContext, TaskQuery,
};
use crate::util;
use crate::workspace::Workspace;
use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tabled::builder::Builder;
use tabled::settings::Style;
use tabled::settings::object::{Columns, Rows};
use tabled::settings::style::Border;
use tabled::settings::{Modify, Width};

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

#[derive(Debug, Clone)]
struct TaskAddSpec {
    source: String,
    project: Option<String>,
    title: Option<String>,
    due: Option<String>,
    deadline: Option<String>,
    labels: Vec<String>,
    priority: Option<String>,
    description: Option<String>,
    note: Option<String>,
    text: Option<String>,
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

impl TaskProvider for PkmsTaskProvider<'_> {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Pkms
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        match query.view {
            TaskListView::All => collect_pkms_list_items(self.context.config),
            TaskListView::Agenda => collect_pkms_agenda_items(self.context.config),
            TaskListView::Today => {
                collect_pkms_agenda_items_for(self.context.config, true, false, false, false)
            }
            TaskListView::Week => {
                collect_pkms_agenda_items_for(self.context.config, false, true, false, false)
            }
            TaskListView::Overdue => {
                collect_pkms_agenda_items_for(self.context.config, false, false, true, false)
            }
            TaskListView::Upcoming { days } => {
                let mut items =
                    collect_pkms_agenda_items_for(self.context.config, false, false, false, true)?;
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
        let columns = input::resolve_columns(args.table.columns.as_deref(), &config.columns);
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
    let columns = resolve_task_table_columns(config, args.table.columns.as_deref());
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
    let pkms = PkmsTaskProvider::new(config);
    let todoist = TodoistTaskProvider::new(config);

    match filters.source {
        SourceSelection::Pkms => pkms.list(&query),
        SourceSelection::Todoist => todoist.list(&query),
        SourceSelection::All => {
            let mut items = pkms.list(&query)?;
            items.extend(todoist.list(&query)?);
            Ok(items)
        }
    }
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
    let columns = resolve_task_table_columns(config, args.table.columns.as_deref());
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
    let columns = resolve_task_table_columns(config, args.table.columns.as_deref());
    print_task_items(ctx, source, items, args.limit, columns.as_deref())
}

fn shortcut_display_source(raw_filters: &[String]) -> Result<SourceSelection> {
    let filters = parse_task_filters(raw_filters)?;
    Ok(filters.source)
}

fn resolve_task_table_columns(
    config: &ResolvedConfig,
    raw_columns: Option<&str>,
) -> Option<Vec<Column>> {
    raw_columns.map(|columns| input::resolve_columns(Some(columns), &config.columns))
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

fn apply_task_filter_criteria(
    config: &ResolvedConfig,
    items: &mut Vec<TaskItem>,
    criteria: &TaskFilterCriteria,
) -> Result<()> {
    let state_filters = crate::commands::task_common::parse_filters(criteria.state.as_deref());
    if !state_filters.is_empty() {
        items.retain(|item| {
            crate::commands::task_common::apply_state_filter(item.state.as_deref(), &state_filters)
        });
    }

    let tags_filters = crate::commands::task_common::parse_filters(criteria.tags.as_deref());
    if !tags_filters.is_empty() {
        items.retain(|item| {
            crate::commands::task_common::apply_tags_filter(&item.tags, &tags_filters)
        });
    }

    let type_filters = crate::commands::task_common::parse_filters(criteria.kind.as_deref());
    if !type_filters.is_empty() {
        items.retain(|item| {
            crate::commands::task_common::apply_type_filter(
                item.scheduled.is_some(),
                item.deadline.is_some(),
                &type_filters,
            )
        });
    }

    if let Some(prio) = &criteria.prio {
        if prio.is_empty() {
            items.retain(|item| item.priority.is_none());
        } else if let Some(target) = prio.chars().next().map(|p| p.to_ascii_uppercase()) {
            items.retain(|item| {
                item.priority
                    .as_deref()
                    .and_then(|priority| priority.chars().next())
                    .is_some_and(|priority| priority.to_ascii_uppercase() == target)
            });
        }
    }

    if let Some(date_filter) = &criteria.date {
        apply_task_date_filter(items, date_filter);
    }

    if let Some(after) = criteria.after {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt >= &after));
    }

    if let Some(before) = criteria.before {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt <= &before));
    }

    let project_filters = crate::commands::task_common::parse_filters(criteria.project.as_deref());
    if !project_filters.is_empty() {
        items.retain(|item| apply_project_filter(item, &project_filters));
    }

    if !criteria.scope.is_empty() {
        let scope_paths = resolve_task_scope_paths(config, &criteria.scope)?;
        items.retain(|item| task_item_in_scope(item, &criteria.scope, &scope_paths));
    }

    Ok(())
}

fn apply_task_date_filter(items: &mut Vec<TaskItem>, date_filter: &TaskDateFilter) {
    let today = Local::now().date_naive();
    match date_filter {
        TaskDateFilter::Exact(date) => {
            items.retain(|item| item_dates(item).iter().any(|item_date| item_date == date));
        }
        TaskDateFilter::Today => {
            items.retain(|item| item_dates(item).contains(&today));
        }
        TaskDateFilter::Week => {
            let cutoff = today + chrono::Duration::days(7);
            items.retain(|item| {
                item_dates(item)
                    .iter()
                    .any(|item_date| *item_date <= cutoff)
            });
        }
        TaskDateFilter::Overdue => items.retain(|item| item.is_overdue),
        TaskDateFilter::Upcoming => {
            items.retain(|item| {
                !item.is_overdue && item_dates(item).iter().any(|item_date| *item_date > today)
            });
        }
    }
}

fn item_dates(item: &TaskItem) -> Vec<NaiveDate> {
    let mut dates: Vec<NaiveDate> = item_datetimes(item)
        .into_iter()
        .map(|dt| dt.date())
        .collect();
    if let Some(daily_file_date) = &item.daily_file_date
        && let Ok(date) = NaiveDate::parse_from_str(daily_file_date, "%Y-%m-%d")
    {
        dates.push(date);
    }
    dates
}

fn item_datetimes(item: &TaskItem) -> Vec<NaiveDateTime> {
    [item.scheduled.as_ref(), item.deadline.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|date| {
            parse_org_date(&date.raw).map(|parsed| {
                let time = parsed
                    .time
                    .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                parsed.base_date.and_time(time)
            })
        })
        .collect()
}

fn apply_project_filter(item: &TaskItem, filters: &[crate::commands::task_common::Filter]) -> bool {
    for filter in filters {
        match filter {
            crate::commands::task_common::Filter::Include(value) => {
                if !task_item_project_matches(item, value) {
                    return false;
                }
            }
            crate::commands::task_common::Filter::Exclude(value) => {
                if task_item_project_matches(item, value) {
                    return false;
                }
            }
        }
    }
    true
}

fn task_item_project_matches(item: &TaskItem, value: &str) -> bool {
    item.project
        .as_deref()
        .is_some_and(|project| project.eq_ignore_ascii_case(value))
        || item
            .project_id
            .as_deref()
            .is_some_and(|project_id| project_id.eq_ignore_ascii_case(value))
}

fn resolve_task_scope_paths(
    config: &ResolvedConfig,
    scope: &[String],
) -> Result<std::collections::HashSet<String>> {
    let workspace = Workspace::load(config)?;
    let db_root = config.resolved_db_root();
    let mut scope_paths = Vec::new();
    for target in scope {
        if let Some(node) = workspace.graph.find_node(target) {
            scope_paths.push(node.path.clone());
            continue;
        }

        let expanded = if let Some(rest) = target.strip_prefix("~/") {
            dirs::home_dir().map(|home| home.join(rest))
        } else {
            None
        };
        let mut matched = false;
        for candidate in [Some(Path::new(target)), expanded.as_deref()]
            .into_iter()
            .flatten()
        {
            for path in [candidate.to_path_buf()]
                .into_iter()
                .chain(candidate.canonicalize().ok())
            {
                if workspace
                    .graph
                    .results
                    .iter()
                    .any(|result| result.path == path)
                {
                    scope_paths.push(path);
                    matched = true;
                    break;
                }
            }
            if matched {
                break;
            }
        }
        if matched {
            continue;
        }

        let joined = db_root.join(target);
        for path in [joined.clone()]
            .into_iter()
            .chain(joined.canonicalize().ok())
        {
            if workspace
                .graph
                .results
                .iter()
                .any(|result| result.path == path)
            {
                scope_paths.push(path);
                break;
            }
        }
    }

    Ok(scope_paths
        .iter()
        .map(|path| path.display().to_string())
        .collect())
}

fn task_item_in_scope(
    item: &TaskItem,
    raw_scope: &[String],
    scope_paths: &std::collections::HashSet<String>,
) -> bool {
    let path = item.path.as_ref().map(|path| path.display().to_string());
    if path.as_ref().is_some_and(|path| {
        scope_paths.contains(path) || raw_scope.iter().any(|scope| scope == path)
    }) {
        return true;
    }

    raw_scope.iter().any(|scope| {
        item.note_uuid.as_deref() == Some(scope.as_str())
            || item.note_title.as_deref() == Some(scope.as_str())
            || item.source_id == *scope
            || item.display_id == *scope
    })
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
    if matches!(filters.source, SourceSelection::Pkms) && !filters.has_criteria() {
        let columns = input::resolve_columns(args.table.columns.as_deref(), &config.columns);
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
    let columns = resolve_task_table_columns(config, args.table.columns.as_deref());
    print_agenda_task_items(ctx, filters.source, items, args.limit, columns.as_deref())
}

fn collect_pkms_list_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let valid_states = config.todo_states();
    let no_filters = Vec::new();
    let mut records = collect_todo_records(
        &workspace.corpus,
        &valid_states,
        &no_filters,
        &no_filters,
        &no_filters,
    );
    assign_canonical_ids(config, &workspace.graph, &mut records);
    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
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

fn collect_pkms_agenda_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    collect_pkms_agenda_items_for(config, false, false, false, false)
}

fn collect_pkms_agenda_items_for(
    config: &ResolvedConfig,
    today_only: bool,
    week: bool,
    overdue: bool,
    upcoming: bool,
) -> Result<Vec<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let today = Local::now().date_naive();
    let valid_states = config.todo_states();
    let closed_states = config.closed_todo_states();
    let no_filters = Vec::new();
    let mut records = collect_agenda_records(
        &workspace.corpus,
        &valid_states,
        &closed_states,
        today,
        &no_filters,
        &no_filters,
        &no_filters,
    );

    if week {
        let cutoff = today + chrono::Duration::days(7);
        records.retain(|item| item_date(item).is_some_and(|date| date <= cutoff));
    } else if today_only {
        records.retain(|item| item_date(item).is_some_and(|date| date == today));
    }

    if overdue {
        records.retain(|item| item.is_overdue);
    }

    if upcoming {
        records.retain(|item| !item.is_overdue && item_date(item).is_some_and(|date| date > today));
    }

    assign_canonical_ids(config, &workspace.graph, &mut records);
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
    let pkms = PkmsTaskProvider::new(config);
    let todoist = TodoistTaskProvider::new(config);

    let collect = |provider: &dyn TaskProvider| match kind {
        MetadataKind::Projects => provider.projects(),
        MetadataKind::Tags => provider.tags(),
    };

    match source {
        SourceSelection::Pkms => collect(&pkms),
        SourceSelection::Todoist => collect(&todoist),
        SourceSelection::All => {
            let mut rows = collect(&pkms)?;
            rows.extend(collect(&todoist)?);
            Ok(rows)
        }
    }
}

fn pkms_project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let mut counts = BTreeMap::new();
    for item in collect_pkms_list_items(config)? {
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
    for item in collect_pkms_list_items(config)? {
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
    let spec = parse_task_add_spec(args)?;
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

fn parse_task_add_spec(args: &TaskAddArgs) -> Result<TaskAddSpec> {
    let mut spec = TaskAddSpec {
        source: "pkms".to_string(),
        project: None,
        title: None,
        due: None,
        deadline: None,
        labels: Vec::new(),
        priority: None,
        description: None,
        note: None,
        text: None,
    };
    let mut text = Vec::new();

    for token in &args.text {
        if apply_task_add_modifier(&mut spec, token)? {
            continue;
        }
        text.push(token.clone());
    }

    if !text.is_empty() {
        set_task_add_option(&mut spec.text, "text", text.join(" "))?;
    }

    Ok(spec)
}

fn apply_task_add_modifier(spec: &mut TaskAddSpec, token: &str) -> Result<bool> {
    let Some((key, value)) = token.split_once(':') else {
        return Ok(false);
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    match key.as_str() {
        "source" | "src" => spec.source = value.to_string(),
        "title" => set_task_add_option(&mut spec.title, "title", value.to_string())?,
        "tag" | "tags" | "label" | "labels" => spec.labels.extend(split_task_add_list(value)),
        "due" | "schedule" | "scheduled" | "sched" | "sch" => {
            set_task_add_option(&mut spec.due, "schedule", value.to_string())?;
        }
        "deadline" | "dead" | "dl" => {
            set_task_add_option(&mut spec.deadline, "deadline", value.to_string())?;
        }
        "project" | "proj" => set_task_add_option(&mut spec.project, "project", value.to_string())?,
        "priority" | "prio" | "pri" => {
            set_task_add_option(&mut spec.priority, "priority", value.to_string())?;
        }
        "description" | "desc" | "body" => {
            set_task_add_option(&mut spec.description, "description", value.to_string())?;
        }
        "note" => set_task_add_option(&mut spec.note, "note", value.to_string())?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn split_task_add_list(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn set_task_add_option<T>(target: &mut Option<T>, name: &str, value: T) -> Result<()> {
    if target.is_some() {
        bail!("task add {name} was provided more than once");
    }
    *target = Some(value);
    Ok(())
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
            path: config.resolve_new_notes_dir().join(format!("{today}.org")),
        });
    }

    let path = config.resolve_new_notes_dir().join(format!("{today}.org"));
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
        std::fs::write(&path, format!("#+title: {title}\n#+filetags: :daily:\n\n"))
            .with_context(|| format!("Failed to create daily note: {}", path.display()))?;
    }
    Ok(PkmsInboxTarget::Daily { path })
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

fn pkms_priority(value: &str) -> Result<char> {
    match value.to_ascii_uppercase().as_str() {
        "A" | "B" | "C" => Ok(value.to_ascii_uppercase().chars().next().unwrap()),
        _ => bail!("Invalid priority '{value}'. Use A, B, or C."),
    }
}

fn validate_pkms_date_arg(name: &str, value: Option<&str>) -> Result<Option<String>> {
    value
        .map(|value| parse_add_date_arg(name, value))
        .transpose()
}

fn parse_add_date_arg(name: &str, value: &str) -> Result<String> {
    if value.eq_ignore_ascii_case("today") || value.eq_ignore_ascii_case("tod") {
        return Ok(Local::now().date_naive().format("%Y-%m-%d").to_string());
    }
    if value.eq_ignore_ascii_case("tomorrow") || value.eq_ignore_ascii_case("tom") {
        return Ok((Local::now().date_naive() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string());
    }
    crate::input::parse_date(Some(value))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid {name} date '{value}'. Use today, tomorrow, tod, tom, or YYYY-MM-DD."
            )
        })
}

fn org_date(date: &str) -> Result<String> {
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    Ok(format!("<{}>", date.format("%Y-%m-%d %a")))
}

fn append_pkms_inbox_entry(target: &PkmsInboxTarget, entry: &str) -> Result<(PathBuf, usize)> {
    match target {
        PkmsInboxTarget::Note(path) => {
            append_org_entry(path, entry).map(|line| (path.clone(), line))
        }
        PkmsInboxTarget::Daily { path } => {
            append_daily_inbox_entry(path, entry).map(|line| (path.clone(), line))
        }
    }
}

fn append_org_entry(path: &Path, entry: &str) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read inbox note: {}", path.display()))?;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.ends_with("\n\n") {
        content.push('\n');
    }
    let line_number = content.lines().count() + 1;
    std::fs::write(path, format!("{content}{entry}"))
        .with_context(|| format!("Failed to write inbox note: {}", path.display()))?;
    Ok(line_number)
}

fn append_daily_inbox_entry(path: &Path, entry: &str) -> Result<usize> {
    let mut content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read daily note: {}", path.display()))?;
    normalize_trailing_newline(&mut content);

    if let Some((_start, end)) = inbox_section_range_from_content(&content) {
        let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
        let insert_idx = end.saturating_sub(1);
        lines.insert(insert_idx, entry.to_string());
        std::fs::write(path, lines.concat())
            .with_context(|| format!("Failed to write daily note: {}", path.display()))?;
        return Ok(insert_idx + 1);
    }

    if !content.ends_with("\n\n") {
        content.push('\n');
    }
    let inbox_heading_line = content.lines().count() + 1;
    content.push_str("* Inbox\n");
    content.push_str(entry);
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write daily note: {}", path.display()))?;
    Ok(inbox_heading_line + 1)
}

fn normalize_trailing_newline(content: &mut String) {
    if !content.ends_with('\n') {
        content.push('\n');
    }
}

fn inbox_section_range(path: &Path) -> Result<Option<(usize, usize)>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read daily note: {}", path.display()))?;
    Ok(inbox_section_range_from_content(&content))
}

fn inbox_section_range_from_content(content: &str) -> Option<(usize, usize)> {
    let mut start = None;
    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;
        let Some(captures) = HEADING_RE.captures(line) else {
            continue;
        };
        let level = captures.get(1).map_or("", |m| m.as_str()).len();
        if level != 1 {
            continue;
        }
        if start.is_some() {
            return Some((start?, line_number));
        }
        let title = captures.get(4).map_or("", |m| m.as_str()).trim();
        if title.eq_ignore_ascii_case("Inbox") {
            start = Some(line_number);
        }
    }
    start.map(|start| (start, content.lines().count() + 1))
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
    let content = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if content.is_empty() || !content.ends_with('\n') {
        let consumed: usize = lines.iter().map(String::len).sum();
        if consumed < content.len() {
            lines.push(content[consumed..].to_string());
        }
    }
    let idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let line = lines
        .get(idx)
        .ok_or_else(|| anyhow::anyhow!("Task line {line_number} no longer exists in {path}"))?
        .clone();
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let body = line.strip_suffix(newline).unwrap_or(&line);
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
        std::fs::write(path, lines.concat())?;
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
    let content = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if content.is_empty() || !content.ends_with('\n') {
        let consumed: usize = lines.iter().map(String::len).sum();
        if consumed < content.len() {
            lines.push(content[consumed..].to_string());
        }
    }
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
    std::fs::write(path, lines.concat())?;
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
    let content = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if content.is_empty() || !content.ends_with('\n') {
        let consumed: usize = lines.iter().map(String::len).sum();
        if consumed < content.len() {
            lines.push(content[consumed..].to_string());
        }
    }
    let heading_idx = line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid task line number: {line_number}"))?;
    let planning_idx = find_planning_line_index(&lines, heading_idx).ok_or_else(|| {
        anyhow::anyhow!("Task does not have a recurring scheduled or deadline date")
    })?;
    lines[planning_idx] = postpone_recurring_planning_line(&lines[planning_idx], new_date)?;
    std::fs::write(path, lines.concat())?;
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
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let body = line.strip_suffix(newline).unwrap_or(line);
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
    if let Some(date) = effective_date(item) {
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

fn item_date(item: &crate::commands::task_index::TaskRecord) -> Option<NaiveDate> {
    item.scheduled_date
        .as_deref()
        .or(item.deadline_date.as_deref())
        .or(item.daily_file_date.as_deref())
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

fn retain_upcoming_task_items(items: &mut Vec<TaskItem>, days: i64) {
    let today = Local::now().date_naive();
    let cutoff = today + chrono::Duration::days(days);
    items.retain(|item| {
        effective_date(item)
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .is_some_and(|date| date > today && date <= cutoff)
    });
}

fn sort_task_items(items: &mut [TaskItem], sort: &str) -> Result<()> {
    let fields = parse_task_sort_fields(sort)?;
    items.sort_by(|a, b| {
        for field in &fields {
            let ord = match *field {
                "priority" => priority_sort_value(a).cmp(&priority_sort_value(b)),
                "date" => effective_date(a).cmp(&effective_date(b)),
                "scheduled" => task_date_value(&a.scheduled).cmp(&task_date_value(&b.scheduled)),
                "deadline" => task_date_value(&a.deadline).cmp(&task_date_value(&b.deadline)),
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

fn task_date_value(date: &Option<crate::tasks::model::TaskDate>) -> Option<&str> {
    date.as_ref().and_then(|date| date.date.as_deref())
}

fn priority_sort_value(item: &TaskItem) -> u8 {
    item.priority
        .as_deref()
        .and_then(|priority| priority.chars().next())
        .map(crate::util::priority_value)
        .unwrap_or(3)
}

fn effective_date(item: &TaskItem) -> Option<&str> {
    item.scheduled
        .as_ref()
        .and_then(|date| date.date.as_deref())
        .or_else(|| item.deadline.as_ref().and_then(|date| date.date.as_deref()))
        .or(item.daily_file_date.as_deref())
}

fn task_item_is_overdue(item: &TaskItem, today: NaiveDate) -> bool {
    item.is_overdue || item_dates(item).into_iter().any(|date| date < today)
}

fn task_item_is_today(item: &TaskItem, today: NaiveDate) -> bool {
    item_dates(item).contains(&today)
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
    print_task_table_sections(&[("", items)], total, source, columns)
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
        if task_item_is_overdue(item, today) {
            overdue.push(item.clone());
        } else if task_item_is_today(item, today) {
            today_items.push(item.clone());
        } else if effective_date(item).is_some() {
            upcoming.push(item.clone());
        }
    }

    overdue.sort_by(|a, b| effective_date(a).cmp(&effective_date(b)));
    today_items.sort_by(|a, b| effective_date(a).cmp(&effective_date(b)));
    upcoming.sort_by(|a, b| effective_date(a).cmp(&effective_date(b)));

    let sections = [
        ("=== Overdue ===", overdue.as_slice()),
        ("=== Today ===", today_items.as_slice()),
        ("=== Upcoming ===", upcoming.as_slice()),
    ];
    print_task_table_sections(&sections, total, source, columns)
}

fn print_task_table_sections(
    sections: &[(&str, &[TaskItem])],
    total: usize,
    source: SourceSelection,
    columns: Option<&[Column]>,
) -> Result<()> {
    let item_count: usize = sections.iter().map(|(_, items)| items.len()).sum();
    if item_count == 0 {
        println!("No tasks found.");
        return Ok(());
    }

    const HEADERS: [&str; 9] = [
        "Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading",
    ];
    let selected_columns = task_table_selected_columns(columns);
    let mut builder = Builder::new();
    builder.push_record(
        selected_columns
            .iter()
            .map(|column| HEADERS[*column])
            .collect::<Vec<_>>(),
    );
    let n_cols = selected_columns.len();
    let empty_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
    let mut section_rows = Vec::new();
    let mut row_idx = 1;
    let mut need_sep = false;

    for (label, items) in sections {
        if items.is_empty() {
            continue;
        }
        if need_sep {
            builder.push_record(empty_row.clone());
            row_idx += 1;
        }
        if !label.is_empty() {
            let mut label_row = empty_row.clone();
            label_row[0] = label.to_string();
            builder.push_record(label_row);
            section_rows.push(row_idx);
            row_idx += 1;
        }
        for item in *items {
            for row in task_table_rows(item, source) {
                builder.push_record(
                    selected_columns
                        .iter()
                        .map(|column| row[*column].clone())
                        .collect::<Vec<_>>(),
                );
                row_idx += 1;
            }
        }
        need_sep = true;
    }
    let all_items = sections
        .iter()
        .flat_map(|(_, items)| items.iter())
        .collect::<Vec<_>>();
    let wrap_widths = task_table_wrap_widths(&all_items, source, &selected_columns);
    let mut table = builder.build();
    table.with(Style::blank());
    table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    for (column, width) in wrap_widths {
        table.with(Modify::new(Columns::one(column)).with(Width::wrap(width).keep_words(true)));
    }
    for section_row in section_rows {
        table.with(
            Modify::new((section_row, 0)).with(tabled::settings::Span::column(n_cols as isize)),
        );
    }
    println!("{table}");
    println!();
    println!("Shown: {}, Total: {} task(s)", item_count, total);
    Ok(())
}

fn task_table_selected_columns(columns: Option<&[Column]>) -> Vec<usize> {
    match columns {
        Some(columns) => columns.iter().map(task_table_column_index).collect(),
        None => (0..9).collect(),
    }
}

fn task_table_column_index(column: &Column) -> usize {
    match column {
        Column::Id => 0,
        Column::Date => 1,
        Column::State => 2,
        Column::Type => 3,
        Column::Prio => 4,
        Column::Tags => 5,
        Column::Note => 7,
        Column::Heading => 8,
    }
}

fn task_table_wrap_widths(
    items: &[&TaskItem],
    source: SourceSelection,
    selected_columns: &[usize],
) -> Vec<(usize, usize)> {
    const HEADERS: [&str; 9] = [
        "Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading",
    ];
    const FIXED_COLUMNS: [usize; 5] = [0, 1, 2, 3, 4];
    const WRAP_COLUMNS: [usize; 4] = [5, 6, 7, 8];
    const DEFAULT_WIDTHS: [(usize, usize); 4] = [(5, 25), (6, 20), (7, 24), (8, 30)];
    const WEIGHTS: [(usize, usize); 4] = [(5, 10), (6, 15), (7, 5), (8, 70)];
    const MIN_WIDTHS: [(usize, usize); 4] = [(5, 4), (6, 7), (7, 4), (8, 20)];

    let Some(term_width) = crate::output::table::terminal_width() else {
        return selected_task_table_default_widths(selected_columns, &DEFAULT_WIDTHS);
    };

    let mut max_widths = HEADERS.map(str::len);
    for item in items {
        for row in task_table_rows(item, source) {
            for (idx, value) in row.iter().enumerate() {
                let width = value
                    .lines()
                    .map(|line| line.chars().count())
                    .max()
                    .unwrap_or(0);
                max_widths[idx] = max_widths[idx].max(width);
            }
        }
    }

    let padding = selected_columns.len().saturating_sub(1) + 2 * selected_columns.len();
    let fixed_width: usize = selected_columns
        .iter()
        .filter(|column| FIXED_COLUMNS.contains(column))
        .map(|idx| max_widths[*idx])
        .sum();
    let available = term_width.saturating_sub(fixed_width + padding);
    let selected_wrap_columns: Vec<usize> = selected_columns
        .iter()
        .copied()
        .filter(|column| WRAP_COLUMNS.contains(column))
        .collect();
    let min_total: usize = selected_wrap_columns
        .iter()
        .map(|column| task_table_lookup(&MIN_WIDTHS, *column))
        .sum();
    if available < min_total {
        return selected_task_table_default_widths(selected_columns, &DEFAULT_WIDTHS);
    }

    let mut widths: Vec<(usize, usize)> = WRAP_COLUMNS
        .iter()
        .filter(|column| selected_columns.contains(column))
        .map(|column| {
            let min = task_table_lookup(&MIN_WIDTHS, *column);
            (*column, min.min(max_widths[*column]))
        })
        .collect();

    let total_weight: usize = WEIGHTS.iter().map(|(_, weight)| *weight).sum();
    loop {
        let used: usize = widths.iter().map(|(_, width)| *width).sum();
        let mut remaining = available.saturating_sub(used);
        if remaining == 0 {
            break;
        }

        let mut changed = false;
        for (column, _) in WEIGHTS.iter().rev() {
            let weight = task_table_lookup(&WEIGHTS, *column);
            if let Some((_, width)) = widths.iter_mut().find(|(candidate, _)| candidate == column) {
                let room = max_widths[*column].saturating_sub(*width);
                let add = (remaining * weight / total_weight)
                    .max(1)
                    .min(room)
                    .min(remaining);
                *width += add;
                remaining -= add;
                changed |= add > 0;
                if remaining == 0 {
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }

    widths
        .into_iter()
        .filter_map(|(column, width)| {
            selected_columns
                .iter()
                .position(|selected| *selected == column)
                .map(|display_column| (display_column, width))
        })
        .collect()
}

fn selected_task_table_default_widths(
    selected_columns: &[usize],
    default_widths: &[(usize, usize)],
) -> Vec<(usize, usize)> {
    default_widths
        .iter()
        .filter_map(|(column, width)| {
            selected_columns
                .iter()
                .position(|selected| selected == column)
                .map(|display_column| (display_column, *width))
        })
        .collect()
}

fn task_table_lookup(values: &[(usize, usize)], column: usize) -> usize {
    values
        .iter()
        .find_map(|(candidate, value)| (*candidate == column).then_some(*value))
        .unwrap_or(0)
}

fn task_table_rows(item: &TaskItem, source: SourceSelection) -> Vec<[String; 9]> {
    let id = task_text_id(item, source);
    let state = item.state.clone().unwrap_or_default();
    let prio = item
        .priority
        .as_deref()
        .map(|priority| format!("[#{priority}]"))
        .unwrap_or_default();
    let tags = item.tags.join(", ");
    let project = item.project.clone().unwrap_or_default();
    let note = item.note_title.clone().unwrap_or_default();
    let heading = item.title.clone();
    let has_both = item.scheduled.is_some() && item.deadline.is_some();
    let mut rows = Vec::new();

    if let Some(date) = &item.scheduled {
        rows.push([
            id.clone(),
            crate::commands::task_common::format_display_datetime(&date.raw),
            state.clone(),
            "SCHED".to_string(),
            prio.clone(),
            tags.clone(),
            project.clone(),
            note.clone(),
            heading.clone(),
        ]);
    }

    if let Some(date) = &item.deadline {
        rows.push([
            if has_both { String::new() } else { id.clone() },
            crate::commands::task_common::format_display_datetime(&date.raw),
            if has_both {
                String::new()
            } else {
                state.clone()
            },
            "DEADL".to_string(),
            if has_both {
                String::new()
            } else {
                prio.clone()
            },
            if has_both {
                String::new()
            } else {
                tags.clone()
            },
            if has_both {
                String::new()
            } else {
                project.clone()
            },
            if has_both {
                String::new()
            } else {
                note.clone()
            },
            if has_both {
                String::new()
            } else {
                heading.clone()
            },
        ]);
    }

    if rows.is_empty() {
        rows.push([
            id,
            item.daily_file_date.clone().unwrap_or_default(),
            state,
            String::new(),
            prio,
            tags,
            project,
            note,
            heading,
        ]);
    }

    rows
}

fn task_text_id(item: &TaskItem, source: SourceSelection) -> String {
    match source {
        SourceSelection::All => match item.source {
            TaskSourceKind::Pkms => format!("p{}", item.source_id),
            TaskSourceKind::Todoist => format!("t{}", item.source_id),
        },
        SourceSelection::Pkms | SourceSelection::Todoist => item.source_id.clone(),
    }
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
