use crate::cli::OutputFormat;
use crate::commands::task_common::{RowItem, print_table_with_empty_message};
use crate::output::{ALL_COLUMNS, Column, OutputContext};
use crate::tasks::filter::SourceSelection;
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::provider::TaskMetadataRow;
use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::Style;

#[derive(Debug, Serialize)]
pub(super) struct TaskStateChangeOutput {
    pub id: String,
    pub path: String,
    pub line_number: usize,
    pub old_state: String,
    pub new_state: String,
    pub dry_run: bool,
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

pub(super) fn print_task_items(
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

pub(super) fn print_agenda_task_items(
    ctx: &OutputContext,
    source: SourceSelection,
    mut items: Vec<TaskItem>,
    limit: Option<usize>,
    columns: Option<&[Column]>,
    today: NaiveDate,
) -> Result<()> {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }

    match ctx.format {
        OutputFormat::Text => print_agenda_task_table(&items, total, source, columns, today),
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
    today: NaiveDate,
) -> Result<()> {
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

pub(super) fn source_name(item: &TaskItem) -> &'static str {
    match item.source {
        TaskSourceKind::Pkms => "pkms",
        TaskSourceKind::Todoist => "todoist",
    }
}
