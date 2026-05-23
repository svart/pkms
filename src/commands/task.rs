use crate::cli::{
    OutputFormat, TaskAddArgs, TaskAgendaArgs, TaskCommand, TaskDoneArgs, TaskListArgs,
    TaskOpenArgs, TaskStateArgs, TaskTargetArgs,
};
use crate::commands::open::OpenOptions;
use crate::commands::show::{HeadingTarget, ShowOptions};
use crate::commands::task_index::{
    assign_canonical_ids, collect_agenda_records, collect_todo_records,
};
use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use crate::parser::HEADING_RE;
use crate::tasks::filter::{SourceSelection, TaskFilters, parse_task_filters};
use crate::tasks::id::TaskId;
use crate::tasks::model::TaskItem;
use crate::tasks::pkms::record_to_task_item;
use crate::workspace::Workspace;
use anyhow::{Result, bail};
use chrono::{Local, NaiveDate};
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::Style;

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
        TaskCommand::Show(args) => run_show(config, ctx, args),
        TaskCommand::Open(args) => run_open(config, ctx, args),
        TaskCommand::State(args) => run_state(config, ctx, args),
        TaskCommand::Done(args) => run_done(config, ctx, args),
        TaskCommand::Add(args) => run_add(config, ctx, args),
    }
}

fn run_list(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskListArgs) -> Result<()> {
    let filters = parse_task_filters(&args.filters)?;
    let mut items = match filters.source {
        SourceSelection::Pkms => collect_pkms_list_items(config)?,
        SourceSelection::Todoist => collect_todoist_items(config, &filters)?,
        SourceSelection::All => {
            let mut items = collect_pkms_list_items(config)?;
            items.extend(collect_todoist_items(config, &filters)?);
            items
        }
    };
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"));
    print_task_items(ctx, items, args.limit)
}

fn run_agenda(config: &ResolvedConfig, ctx: &OutputContext, args: &TaskAgendaArgs) -> Result<()> {
    let filters = parse_task_filters(&args.filters)?;
    let todoist_filters = todoist_agenda_filters(&filters, args);
    if matches!(filters.source, SourceSelection::Todoist) {
        let mut items = collect_todoist_items(config, &todoist_filters)?;
        sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"));
        return print_task_items(ctx, items, args.limit);
    }

    if matches!(filters.source, SourceSelection::All) {
        let mut items = collect_pkms_agenda_items(config, args)?;
        items.extend(collect_todoist_items(config, &todoist_filters)?);
        sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"));
        return print_task_items(ctx, items, args.limit);
    }

    let mut items = collect_pkms_agenda_items(config, args)?;
    sort_task_items(&mut items, args.sort.as_deref().unwrap_or("priority"));
    print_task_items(ctx, items, args.limit)
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
        None
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
    Ok(tasks
        .into_iter()
        .map(crate::tasks::todoist::task_to_item)
        .collect())
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
    let item = crate::tasks::todoist::task_to_item(client.get_task(id)?);
    match ctx.format {
        OutputFormat::Text => print_task_table(&[item], 1),
        OutputFormat::Json => ctx.print_json(&item),
        OutputFormat::Ndjson => ctx.print_ndjson(&[item]),
    }
}

#[cfg(not(feature = "todoist"))]
fn show_todoist_task(_config: &ResolvedConfig, _ctx: &OutputContext, _id: &str) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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
    quick_add_todoist_task(config, ctx, args)
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
fn quick_add_todoist_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    args: &TaskAddArgs,
) -> Result<()> {
    #[derive(Serialize)]
    struct AddOutput {
        source: &'static str,
        remote_id: Option<String>,
        created: bool,
    }

    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let text = quick_add_text(&args.text, args.project.as_deref());
    let response = client.quick_add(&text)?;
    let remote_id = response
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let output = AddOutput {
        source: "todoist",
        remote_id,
        created: true,
    };
    match ctx.format {
        OutputFormat::Text => {
            if let Some(id) = output.remote_id {
                println!("Created Todoist task: todoist:{id}");
            } else {
                println!("Created Todoist task");
            }
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(&output),
        OutputFormat::Ndjson => ctx.print_ndjson(&[output]),
    }
}

#[cfg(not(feature = "todoist"))]
fn quick_add_todoist_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _args: &TaskAddArgs,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
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
        std::cmp::Ordering::Equal
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
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
) -> Result<()> {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }

    match ctx.format {
        OutputFormat::Text => print_task_table(&items, total),
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

fn print_task_table(items: &[TaskItem], total: usize) -> Result<()> {
    if items.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    let mut builder = Builder::new();
    builder.push_record([
        "Id", "Source", "Date", "State", "Prio", "Tags", "Project", "Task",
    ]);
    for item in items {
        builder.push_record([
            item.display_id.clone(),
            source_name(item).to_string(),
            effective_date(item).unwrap_or("").to_string(),
            item.state.clone().unwrap_or_default(),
            item.priority
                .as_deref()
                .map(|priority| format!("[#{priority}]"))
                .unwrap_or_default(),
            item.tags.join(", "),
            item.project.clone().unwrap_or_default(),
            item.title.clone(),
        ]);
    }
    let mut table = builder.build();
    table.with(Style::blank());
    println!("{table}");
    println!();
    println!("Shown: {}, Total: {} task(s)", items.len(), total);
    Ok(())
}

fn source_name(item: &TaskItem) -> &'static str {
    match item.source {
        crate::tasks::model::TaskSourceKind::Pkms => "pkms",
        crate::tasks::model::TaskSourceKind::Todoist => "todoist",
    }
}
