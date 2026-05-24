use crate::cli::{
    OutputFormat, TaskAddArgs, TaskAgendaArgs, TaskCommand, TaskDoneArgs, TaskListArgs,
    TaskOpenArgs, TaskPostponeArgs, TaskScheduleArgs, TaskShortcutArgs, TaskStateArgs,
    TaskTargetArgs, TaskUpcomingArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::commands::task_index::{
    assign_canonical_ids, collect_agenda_records, collect_todo_records,
};
use crate::config::ResolvedConfig;
use crate::input;
use crate::output::OutputContext;
use crate::parser::HEADING_RE;
use crate::tasks::filter::{SourceSelection, TaskFilters, parse_task_filters};
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms::record_to_task_item;
use crate::workspace::Workspace;
use anyhow::{Result, bail};
use chrono::{Local, NaiveDate};
use serde::Serialize;
use std::collections::BTreeMap;
use tabled::builder::Builder;
use tabled::settings::Style;
use tabled::settings::object::{Columns, Rows};
use tabled::settings::style::Border;
use tabled::settings::{Modify, Width};

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

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, command: &TaskCommand) -> Result<()> {
    match command {
        TaskCommand::List(args) => run_list(config, ctx, args),
        TaskCommand::Agenda(args) => run_agenda(config, ctx, args),
        TaskCommand::Today(args) => run_shortcut(config, ctx, args, ShortcutKind::Today),
        TaskCommand::Overdue(args) => run_shortcut(config, ctx, args, ShortcutKind::Overdue),
        TaskCommand::Upcoming(args) => run_upcoming(config, ctx, args),
        TaskCommand::Inbox(args) => run_shortcut(config, ctx, args, ShortcutKind::Inbox),
        TaskCommand::Show(args) => run_show(config, ctx, args),
        TaskCommand::Open(args) => run_open(config, ctx, args),
        TaskCommand::State(args) => run_state(config, ctx, args),
        TaskCommand::Done(args) => run_done(config, ctx, args),
        TaskCommand::Add(args) => run_add(config, ctx, args),
        TaskCommand::Postpone(args) => run_postpone(config, ctx, args),
        TaskCommand::Schedule(args) => run_schedule(config, ctx, args),
    }
}

#[derive(Debug, Clone, Copy)]
enum ShortcutKind {
    Today,
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
    if matches!(filters.source, SourceSelection::Pkms) {
        let columns = input::resolve_columns(args.table.columns.as_deref(), &config.columns);
        return crate::commands::todo::run(
            config,
            ctx,
            &crate::commands::todo::TodoOptions {
                state: None,
                tags: None,
                kind: None,
                sort: args.sort.clone(),
                limit: args.limit,
                group: None,
                scope: Vec::new(),
                after: None,
                before: None,
                prio: None,
                line_sep: args.table.line_sep,
                columns,
            },
        );
    }

    let mut items = match filters.source {
        SourceSelection::Todoist => collect_todoist_items(config, &filters)?,
        SourceSelection::All => {
            let mut items = collect_pkms_list_items(config)?;
            items.extend(collect_todoist_items(config, &filters)?);
            items
        }
        SourceSelection::Pkms => unreachable!("PKMS task list is handled by todo::run"),
    };
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"));
    print_task_items(ctx, filters.source, items, args.limit)
}

fn run_shortcut(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskShortcutArgs,
    kind: ShortcutKind,
) -> Result<()> {
    let mut items = collect_shortcut_items(config, &args.filters, kind)?;
    let source = shortcut_display_source(&args.filters, kind)?;
    sort_task_items(&mut items, "priority");
    print_task_items(ctx, source, items, args.limit)
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
    let source =
        shortcut_display_source(&args.filters, ShortcutKind::Upcoming { days: args.days })?;
    sort_task_items(&mut items, "priority");
    print_task_items(ctx, source, items, args.limit)
}

fn shortcut_display_source(raw_filters: &[String], kind: ShortcutKind) -> Result<SourceSelection> {
    let mut filters = parse_task_filters(raw_filters)?;
    if matches!(kind, ShortcutKind::Inbox) && raw_filters.is_empty() {
        filters.source = SourceSelection::Todoist;
    }
    Ok(filters.source)
}

fn collect_shortcut_items(
    config: &ResolvedConfig,
    raw_filters: &[String],
    kind: ShortcutKind,
) -> Result<Vec<TaskItem>> {
    let mut filters = parse_task_filters(raw_filters)?;
    if matches!(kind, ShortcutKind::Inbox) && raw_filters.is_empty() {
        filters.source = SourceSelection::Todoist;
    }
    if matches!(kind, ShortcutKind::Inbox) && matches!(filters.source, SourceSelection::Pkms) {
        bail!("PKMS inbox tasks are not supported yet. Use source:todoist.");
    }

    let todoist_filters = shortcut_todoist_filters(&filters, kind);
    let items = match filters.source {
        SourceSelection::Pkms => collect_pkms_shortcut_items(config, kind)?,
        SourceSelection::Todoist => collect_todoist_items(config, &todoist_filters)?,
        SourceSelection::All => {
            let mut items = if matches!(kind, ShortcutKind::Inbox) {
                Vec::new()
            } else {
                collect_pkms_shortcut_items(config, kind)?
            };
            items.extend(collect_todoist_items(config, &todoist_filters)?);
            items
        }
    };
    Ok(items)
}

fn run_agenda(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskAgendaArgs) -> Result<()> {
    let filters = parse_task_filters(&args.filters)?;
    if matches!(filters.source, SourceSelection::Pkms) {
        let columns = input::resolve_columns(args.table.columns.as_deref(), &config.columns);
        return crate::commands::agenda::run(
            config,
            ctx,
            &crate::commands::agenda::AgendaOptions {
                state: None,
                tags: None,
                kind: None,
                prio: None,
                overdue: args.overdue,
                upcoming: args.upcoming,
                date: None,
                sort: args.sort.clone(),
                limit: args.limit,
                today: args.today,
                week: args.week,
                line_sep: args.table.line_sep,
                columns,
            },
        );
    }

    let todoist_filters = todoist_agenda_filters(&filters, args);
    if matches!(filters.source, SourceSelection::Todoist) {
        let mut items = collect_todoist_items(config, &todoist_filters)?;
        sort_task_items(&mut items, args.sort.as_deref().unwrap_or("date,priority"));
        return print_task_items(ctx, filters.source, items, args.limit);
    }

    if matches!(filters.source, SourceSelection::All) {
        let mut items = collect_pkms_agenda_items(config, args)?;
        items.extend(collect_todoist_items(config, &todoist_filters)?);
        sort_task_items(&mut items, args.sort.as_deref().unwrap_or("date,priority"));
        return print_task_items(ctx, filters.source, items, args.limit);
    }

    let mut items = collect_pkms_agenda_items(config, args)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("date,priority"));
    print_task_items(ctx, filters.source, items, args.limit)
}

fn collect_pkms_shortcut_items(
    config: &ResolvedConfig,
    kind: ShortcutKind,
) -> Result<Vec<TaskItem>> {
    let args = TaskAgendaArgs {
        filters: Vec::new(),
        today: matches!(kind, ShortcutKind::Today),
        week: false,
        overdue: matches!(kind, ShortcutKind::Overdue),
        upcoming: matches!(kind, ShortcutKind::Upcoming { .. }),
        sort: None,
        limit: None,
        table: crate::cli::TaskTableArgs {
            line_sep: false,
            columns: None,
        },
    };
    let mut items = collect_pkms_agenda_items(config, &args)?;
    if let ShortcutKind::Upcoming { days } = kind {
        retain_upcoming_task_items(&mut items, days);
    }
    Ok(items)
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

fn collect_pkms_agenda_items(
    config: &ResolvedConfig,
    args: &TaskAgendaArgs,
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

    if args.week {
        let cutoff = today + chrono::Duration::days(7);
        records.retain(|item| item_date(item).is_some_and(|date| date <= cutoff));
    } else if args.today {
        records.retain(|item| item_date(item).is_some_and(|date| date == today));
    }

    if args.overdue {
        records.retain(|item| item.is_overdue);
    }

    if args.upcoming {
        records.retain(|item| !item.is_overdue && item_date(item).is_some_and(|date| date > today));
    }

    assign_canonical_ids(config, &workspace.graph, &mut records);
    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
}

fn todoist_agenda_filters(filters: &TaskFilters, args: &TaskAgendaArgs) -> TaskFilters {
    let todoist_filter = filters
        .todoist_filter
        .clone()
        .or_else(|| todoist_agenda_filter(args).map(str::to_string));
    TaskFilters {
        source: filters.source,
        todoist_filter,
    }
}

fn todoist_agenda_filter(args: &TaskAgendaArgs) -> Option<&'static str> {
    if args.upcoming {
        Some("due after: today")
    } else if args.overdue {
        Some("overdue")
    } else if args.today {
        Some("today")
    } else if args.week {
        Some("next 7 days")
    } else {
        Some("!no date")
    }
}

fn shortcut_todoist_filters(filters: &TaskFilters, kind: ShortcutKind) -> TaskFilters {
    let todoist_filter = filters
        .todoist_filter
        .clone()
        .or_else(|| shortcut_todoist_filter(kind));
    TaskFilters {
        source: filters.source,
        todoist_filter,
    }
}

fn shortcut_todoist_filter(kind: ShortcutKind) -> Option<String> {
    match kind {
        ShortcutKind::Today => Some("today".to_string()),
        ShortcutKind::Overdue => Some("overdue".to_string()),
        ShortcutKind::Upcoming { days } => Some(format!("due after: today & next {days} days")),
        ShortcutKind::Inbox => Some("#Inbox".to_string()),
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
    }
}

fn run_open(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskOpenArgs) -> Result<()> {
    let TaskId::Pkms(id) = args.id.parse::<TaskId>()? else {
        bail!("Todoist task source is not implemented yet");
    };
    crate::commands::open::run(
        config,
        ctx,
        &OpenOptions {
            targets: vec![id.to_string()],
            editor: args.editor.clone(),
            line: args.line,
        },
    )
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
        OutputFormat::Text => print_task_table(&[item], 1, SourceSelection::Todoist),
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
    let mut rows = Vec::new();
    if matches!(filters.source, SourceSelection::Pkms | SourceSelection::All) {
        rows.extend(pkms_project_rows(config)?);
    }
    if matches!(
        filters.source,
        SourceSelection::Todoist | SourceSelection::All
    ) {
        rows.extend(todoist_project_rows(config)?);
    }
    sort_metadata_rows(&mut rows);
    print_metadata_rows(ctx, "project", &rows)
}

fn run_tags(config: &ResolvedConfig, ctx: &OutputContext, filters: &[String]) -> Result<()> {
    let filters = parse_task_filters(filters)?;
    if filters.todoist_filter.is_some() {
        bail!("Todoist metadata commands do not accept todoist.filter.");
    }
    let mut rows = Vec::new();
    if matches!(filters.source, SourceSelection::Pkms | SourceSelection::All) {
        rows.extend(pkms_tag_rows(config)?);
    }
    if matches!(
        filters.source,
        SourceSelection::Todoist | SourceSelection::All
    ) {
        rows.extend(todoist_label_rows(config)?);
    }
    sort_metadata_rows(&mut rows);
    print_metadata_rows(ctx, "tag", &rows)
}

#[derive(Debug, Clone, Serialize)]
struct TaskMetadataRow {
    source: TaskSourceKind,
    id: String,
    name: String,
    count: Option<usize>,
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
    set_pkms_task_state(config, ctx, &args.id, &args.state, args.dry_run)
}

fn run_done(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskDoneArgs) -> Result<()> {
    if let TaskId::Todoist(id) = args.id.parse::<TaskId>()? {
        return close_todoist_task(config, ctx, &id, args.dry_run);
    }
    let closed_state = config
        .closed_todo_states()
        .first()
        .cloned()
        .unwrap_or_else(|| "DONE".to_string());
    set_pkms_task_state(config, ctx, &args.id, &closed_state, args.dry_run)
}

fn run_add(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskAddArgs) -> Result<()> {
    if !args.source.eq_ignore_ascii_case("todoist") {
        bail!("PKMS task creation is not supported");
    }
    add_todoist_task(config, ctx, args)
}

fn run_postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskPostponeArgs,
) -> Result<()> {
    let id = todoist_only_id(&args.id)?;
    mutate_todoist_task(
        config,
        ctx,
        &id,
        "postpone",
        serde_json::json!({ "due_date": parse_mutation_due_date(&args.to)? }),
    )
}

fn run_schedule(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskScheduleArgs,
) -> Result<()> {
    let id = todoist_only_id(&args.id)?;
    let due = if args.due.eq_ignore_ascii_case("none") {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(parse_mutation_due_date(&args.due)?)
    };
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

fn todoist_only_id(id: &str) -> Result<String> {
    let TaskId::Todoist(id) = id.parse::<TaskId>()? else {
        bail!("This task command currently supports Todoist task ids only.");
    };
    Ok(id)
}

#[cfg(feature = "todoist")]
#[derive(Debug, Clone)]
struct PkmsNoteLink {
    uuid: String,
    title: String,
}

#[cfg(feature = "todoist")]
fn resolve_pkms_note_link(config: &ResolvedConfig, target: &str) -> Result<PkmsNoteLink> {
    let graph = crate::graph::Graph::load(config)?;
    let node = graph.resolve_target(target)?;
    Ok(PkmsNoteLink {
        uuid: node.uuid.clone(),
        title: node.title.clone(),
    })
}

#[cfg(feature = "todoist")]
fn description_with_pkms_note_marker(description: Option<&str>, uuid: &str) -> String {
    let marker = format!("{PKMS_NOTE_MARKER_PREFIX}{uuid}");
    match description
        .map(str::trim_end)
        .filter(|value| !value.is_empty())
    {
        Some(description) if description.contains(&marker) => description.to_string(),
        Some(description) => format!("{description}\n\n{marker}"),
        None => marker,
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
        bail!("Todoist task source is not implemented yet");
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

#[cfg(feature = "todoist")]
fn add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskAddArgs,
) -> Result<()> {
    if is_structured_add(args) {
        return create_structured_todoist_task(config, ctx, args);
    }
    quick_add_todoist_task(config, ctx, args)
}

#[cfg(not(feature = "todoist"))]
fn add_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _args: &TaskAddArgs,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn is_structured_add(args: &TaskAddArgs) -> bool {
    args.title.is_some()
        || args.due.is_some()
        || args.deadline.is_some()
        || !args.label.is_empty()
        || args.priority.is_some()
        || args.description.is_some()
        || args.note.is_some()
}

#[cfg(feature = "todoist")]
fn create_structured_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskAddArgs,
) -> Result<()> {
    if args.title.is_some() && args.text.is_some() {
        bail!("Structured Todoist task creation uses --title or positional text, not both.");
    }
    let title = args
        .title
        .as_deref()
        .or(args.text.as_deref())
        .filter(|title| !title.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Structured Todoist task creation requires --title"))?;
    let linked_note = args
        .note
        .as_deref()
        .map(|target| resolve_pkms_note_link(config, target))
        .transpose()?;
    let description = match linked_note.as_ref() {
        Some(note) => Some(description_with_pkms_note_marker(
            args.description.as_deref(),
            &note.uuid,
        )),
        None => args.description.clone(),
    };
    let due_date = validate_date_arg("due", args.due.as_deref())?;
    let deadline_date = validate_date_arg("deadline", args.deadline.as_deref())?;
    let priority = args
        .priority
        .as_deref()
        .map(todoist_create_priority)
        .transpose()?;
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let metadata = crate::tasks::todoist::TodoistMetadata::new(client.list_projects()?);
    let project_id = args
        .project
        .as_deref()
        .map(|project| metadata.resolve_project_id(project))
        .transpose()?;
    let request = crate::tasks::todoist::TodoistCreateTaskRequest {
        content: title.to_string(),
        description,
        project_id,
        labels: args.label.clone(),
        priority,
        due_date,
        deadline_date,
    };

    let task = client.create_task(&request)?;
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, Some(&metadata));
    if let Some(note) = linked_note {
        item.note_uuid = Some(note.uuid);
        item.note_title = Some(note.title);
    } else {
        enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    }
    print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
fn quick_add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskAddArgs,
) -> Result<()> {
    let text = args
        .text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Todoist Quick Add requires task text or --title"))?;
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let text = quick_add_text(text, args.project.as_deref());
    let response = client.quick_add(&text)?;
    let id = todoist_created_task_id(&response)?;
    let task = client.get_task(&id)?;
    let metadata = crate::tasks::todoist::TodoistMetadata::new(client.list_projects()?);
    let mut item = crate::tasks::todoist::task_to_item_with_metadata(task, Some(&metadata));
    enrich_todoist_items_with_pkms_notes(config, std::slice::from_mut(&mut item))?;
    print_add_output(ctx, item)
}

#[cfg(feature = "todoist")]
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
                "Changed Todoist task: {} (action {}; id {})",
                output.item.title, action, output.item.display_id
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

#[cfg(feature = "todoist")]
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
        "Created Todoist task: {} ({})",
        item.title,
        details.join("; ")
    );
}

#[cfg(feature = "todoist")]
fn validate_date_arg(name: &str, value: Option<&str>) -> Result<Option<String>> {
    value
        .map(|value| {
            crate::input::parse_date(Some(value))
                .map(|date| date.format("%Y-%m-%d").to_string())
                .ok_or_else(|| anyhow::anyhow!("Invalid {name} date '{value}'. Use YYYY-MM-DD."))
        })
        .transpose()
}

fn parse_mutation_due_date(value: &str) -> Result<String> {
    if value.eq_ignore_ascii_case("tomorrow") {
        return Ok((Local::now().date_naive() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string());
    }
    crate::input::parse_date(Some(value))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid due date '{value}'. Use tomorrow or YYYY-MM-DD."))
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

fn sort_task_items(items: &mut [TaskItem], sort: &str) {
    let fields: Vec<&str> = sort.split(',').map(|field| field.trim()).collect();
    items.sort_by(|a, b| {
        for field in &fields {
            let ord = match *field {
                "priority" => priority_sort_value(a).cmp(&priority_sort_value(b)),
                "date" => effective_date(a).cmp(&effective_date(b)),
                "source" => source_name(a).cmp(source_name(b)),
                "state" => a.state.cmp(&b.state),
                "task" | "title" => a.title.cmp(&b.title),
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
}

fn print_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
) -> Result<()> {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }

    match ctx.format {
        OutputFormat::Text => print_task_table(&items, total, source),
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

fn print_task_table(items: &[TaskItem], total: usize, source: SourceSelection) -> Result<()> {
    if items.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    const HEADERS: [&str; 9] = [
        "Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading",
    ];
    let mut builder = Builder::new();
    builder.push_record(HEADERS);
    for item in items {
        for row in task_table_rows(item, source) {
            builder.push_record(row);
        }
    }
    let mut table = builder.build();
    table.with(Style::blank());
    table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    table.with(Modify::new(Columns::one(5)).with(Width::wrap(25).keep_words(true)));
    table.with(Modify::new(Columns::one(6)).with(Width::wrap(20).keep_words(true)));
    table.with(Modify::new(Columns::one(7)).with(Width::wrap(24).keep_words(true)));
    table.with(Modify::new(Columns::one(8)).with(Width::wrap(30).keep_words(true)));
    println!("{table}");
    println!();
    println!("Shown: {}, Total: {} task(s)", items.len(), total);
    Ok(())
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
            String::new(),
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
