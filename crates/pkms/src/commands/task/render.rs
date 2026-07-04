use crate::cli::OutputFormat;
use crate::commands::task_common::{
    AgendaWindow, RowItem, RowSeparatorMode, TaskGroupField, agenda_day_section_label, apply_limit,
    print_table_with_empty_message,
};
use crate::output::{ALL_COLUMNS, Column, OutputContext, terminal_markup};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime};
use pkms_task::filter::SourceSelection;
use pkms_task::model::{TaskItem, TaskPriority, TaskProperty, TaskSourceKind};
#[cfg(feature = "todoist")]
pub(super) use pkms_task::mutation::TaskModChange;
pub(super) use pkms_task::mutation::{TaskModOutput, TaskStateChangeOutput};
use pkms_task::provider::TaskMetadataRow;
use serde::Serialize;
use std::collections::BTreeMap;
use std::process::ExitCode;
use tabled::builder::Builder;
use tabled::settings::Style;

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
            SourceSelection::All => self.item.display_id.clone(),
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

pub(super) fn print_metadata_rows(
    ctx: &OutputContext,
    kind: &str,
    rows: &[TaskMetadataRow],
) -> Result<()> {
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

pub(super) fn print_state_change(
    ctx: &OutputContext,
    output: &TaskStateChangeOutput,
) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            let action = if output.dry_run {
                "Would change"
            } else {
                "Changed"
            };
            print_task_title(&output.title);
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

pub(super) fn print_add_output(ctx: &OutputContext, item: TaskItem) -> Result<()> {
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

pub(super) fn print_mutation_output(
    ctx: &OutputContext,
    action: &'static str,
    item: TaskItem,
) -> Result<()> {
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
            print_task_title(&output.item.title);
            println!(
                "Changed {} task (action {}; id {})",
                source_name(&output.item),
                action,
                output.item.display_id
            );
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(&output),
        OutputFormat::Ndjson => ctx.print_ndjson(&[output]),
    }
}

pub(super) fn print_mod_output(
    ctx: &OutputContext,
    output: TaskModOutput,
    today: NaiveDate,
) -> Result<ExitCode> {
    let exit_code = if output.changed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };
    match ctx.format {
        OutputFormat::Text => {
            if output.changed {
                if let Some(item) = &output.item {
                    print_task_title(&item.title);
                }
                for change in &output.changes {
                    let old = display_mod_value(change.property, change.old.as_deref(), today);
                    let new = display_mod_value(change.property, change.new.as_deref(), today);
                    println!(
                        "{}: {} -> {}: {}",
                        change.property,
                        format_mod_value(change.property, &old),
                        change.property,
                        format_mod_value(change.property, &new)
                    );
                }
            } else {
                println!("Nothing changed");
            }
            Ok(exit_code)
        }
        OutputFormat::Json => {
            ctx.print_json(&output)?;
            Ok(exit_code)
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&[output])?;
            Ok(exit_code)
        }
    }
}

fn print_task_title(title: &str) {
    let title = terminal_markup::format_if_terminal_supported(title);
    println!("Task: {title}");
}

fn display_mod_value(property: TaskProperty, value: Option<&str>, today: NaiveDate) -> String {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return "None".to_string();
    };
    if matches!(property, TaskProperty::Scheduled | TaskProperty::Deadline)
        && let Some(parsed) = pkms_org::org_date::parse_org_date(value)
    {
        let date = parsed.base_date.format("%Y-%m-%d").to_string();
        let day = if parsed.base_date == today {
            format!("Today ({date})")
        } else if parsed.base_date == today + chrono::Duration::days(1) {
            format!("Tomorrow ({date})")
        } else {
            date
        };
        if let Some(time) = parsed.time {
            return format!("{day} {}", time.format("%H:%M"));
        }
        return day;
    }
    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M") {
        return datetime.format("%Y-%m-%d %H:%M").to_string();
    }
    value.replace('\n', "\\n")
}

fn format_mod_value(property: TaskProperty, value: &str) -> String {
    if property == TaskProperty::Title {
        terminal_markup::format_if_terminal_supported(value)
    } else {
        value.to_string()
    }
}

fn print_created_task(item: &TaskItem) {
    let mut details = vec![format!("id {}", item.display_id)];
    if let Some(date) = item.effective_date() {
        details.push(format!("date {date}"));
    }
    if let Some(priority) = item.priority.map(TaskPriority::as_char) {
        details.push(format!("priority {priority}"));
    }
    if let Some(project) = item.project.as_deref() {
        details.push(format!("project {project}"));
    }
    if !item.tags.is_empty() {
        details.push(format!("labels {}", item.tags.join(", ")));
    }
    let title = terminal_markup::format_if_terminal_supported(&item.title);
    println!(
        "Created {} task: {} ({})",
        source_display_name(item),
        title,
        details.join("; ")
    );
}

fn source_display_name(item: &TaskItem) -> &'static str {
    match item.source {
        TaskSourceKind::Pkms => "PKMS",
        TaskSourceKind::Todoist => "Todoist",
    }
}

pub(super) fn print_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
    table: TaskTableRenderOptions<'_>,
) -> Result<()> {
    let total = apply_limit(&mut items, limit);

    match ctx.format {
        OutputFormat::Text => print_task_table(&items, total, source, table),
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

pub(super) fn print_grouped_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    group_field: TaskGroupField,
    groups: BTreeMap<String, Vec<TaskItem>>,
    total: usize,
    table: TaskTableRenderOptions<'_>,
) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => print_grouped_task_table(&groups, total, source, table),
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct GroupedTaskListOutput {
                total: usize,
                group_field: String,
                groups: BTreeMap<String, Vec<TaskItem>>,
            }

            let shown = groups.values().map(Vec::len).sum();
            ctx.print_json(&GroupedTaskListOutput {
                total: shown,
                group_field: group_field.as_str().to_string(),
                groups,
            })
        }
        OutputFormat::Ndjson => {
            for (group_key, group_items) in groups {
                for item in group_items {
                    let mut json_item = serde_json::to_value(item)?;
                    json_item.as_object_mut().unwrap().insert(
                        "group".to_string(),
                        serde_json::Value::String(group_key.clone()),
                    );
                    println!("{}", serde_json::to_string(&json_item)?);
                }
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct TaskTableRenderOptions<'a> {
    pub(super) columns: Option<&'a [Column]>,
    pub(super) row_separators: RowSeparatorMode,
}

pub(super) struct AgendaTaskRenderOptions<'a> {
    pub(super) table: TaskTableRenderOptions<'a>,
    pub(super) today: NaiveDate,
    pub(super) window: AgendaWindow,
}

struct AgendaSection {
    label: String,
    items: Vec<TaskItem>,
}

impl AgendaSection {
    fn new(label: impl Into<String>, items: Vec<TaskItem>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

pub(super) fn print_agenda_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
    opts: AgendaTaskRenderOptions<'_>,
) -> Result<()> {
    let total = apply_limit(&mut items, limit);

    match ctx.format {
        OutputFormat::Text => {
            print_agenda_task_table(&items, total, source, opts.today, opts.window, opts.table)
        }
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

pub(super) fn print_task_table(
    items: &[TaskItem],
    total: usize,
    source: SourceSelection,
    table: TaskTableRenderOptions<'_>,
) -> Result<()> {
    let rows = task_rows(items, source);
    let sections = [("", rows.as_slice())];
    let footer = format!("Shown: {}, Total: {} task(s)", rows.len(), total);
    print_table_with_empty_message(
        &sections,
        task_columns(table.columns),
        table.row_separators,
        &footer,
        "No tasks found.",
    );
    Ok(())
}

fn print_grouped_task_table(
    groups: &BTreeMap<String, Vec<TaskItem>>,
    total: usize,
    source: SourceSelection,
    table: TaskTableRenderOptions<'_>,
) -> Result<()> {
    let labels: Vec<String> = groups
        .iter()
        .map(|(key, items)| format!("{key} ({})", items.len()))
        .collect();
    let rows: Vec<Vec<TaskRow<'_>>> = groups
        .values()
        .map(|items| task_rows(items, source))
        .collect();
    let sections: Vec<(&str, &[TaskRow<'_>])> = labels
        .iter()
        .zip(rows.iter())
        .map(|(label, rows)| (label.as_str(), rows.as_slice()))
        .collect();
    let shown = groups.values().map(Vec::len).sum::<usize>();
    let footer = format!("Shown: {shown}, Total: {total} task(s)");
    print_table_with_empty_message(
        &sections,
        task_columns(table.columns),
        table.row_separators,
        &footer,
        "No tasks found.",
    );
    Ok(())
}

fn print_agenda_task_table(
    items: &[TaskItem],
    total: usize,
    source: SourceSelection,
    today: NaiveDate,
    window: AgendaWindow,
    table: TaskTableRenderOptions<'_>,
) -> Result<()> {
    let agenda_sections = build_agenda_sections(items, today, window);
    let rows: Vec<Vec<TaskRow<'_>>> = agenda_sections
        .iter()
        .map(|section| task_rows(&section.items, source))
        .collect();
    let sections: Vec<(&str, &[TaskRow<'_>])> = agenda_sections
        .iter()
        .zip(rows.iter())
        .map(|(section, rows)| (section.label.as_str(), rows.as_slice()))
        .collect();
    let item_count = agenda_sections
        .iter()
        .map(|section| section.items.len())
        .sum::<usize>();
    let footer = format!("Shown: {}, Total: {} task(s)", item_count, total);
    print_table_with_empty_message(
        &sections,
        task_columns(table.columns),
        table.row_separators,
        &footer,
        "No tasks found.",
    );
    Ok(())
}

fn build_agenda_sections(
    items: &[TaskItem],
    today: NaiveDate,
    window: AgendaWindow,
) -> Vec<AgendaSection> {
    match window {
        AgendaWindow::Sections => {
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

            vec![
                AgendaSection::new("Overdue", overdue),
                AgendaSection::new("Today", today_items),
                AgendaSection::new("Upcoming", upcoming),
            ]
        }
        AgendaWindow::Days(days) => {
            let mut overdue = Vec::new();
            let mut daily_items: BTreeMap<i64, Vec<TaskItem>> = BTreeMap::new();

            for item in items {
                if item.is_overdue_on(today) {
                    overdue.push(item.clone());
                } else {
                    for date in item.dates() {
                        let offset = (date - today).num_days();
                        if (0..days).contains(&offset) {
                            daily_items.entry(offset).or_default().push(item.clone());
                            break;
                        }
                    }
                }
            }

            overdue.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
            for items in daily_items.values_mut() {
                items.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
            }

            let mut sections = vec![AgendaSection::new("Overdue", overdue)];
            for (offset, items) in daily_items {
                let date = today + chrono::Duration::days(offset);
                sections.push(AgendaSection::new(
                    agenda_day_section_label(today, date),
                    items,
                ));
            }
            sections
        }
    }
}

fn task_rows(items: &[TaskItem], source: SourceSelection) -> Vec<TaskRow<'_>> {
    items.iter().map(|item| TaskRow { item, source }).collect()
}

fn task_columns(columns: Option<&[Column]>) -> &[Column] {
    columns.unwrap_or(ALL_COLUMNS.as_slice())
}

pub(super) fn source_name(item: &TaskItem) -> &'static str {
    match item.source {
        TaskSourceKind::Pkms => "pkms",
        TaskSourceKind::Todoist => "todoist",
    }
}
