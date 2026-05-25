use crate::cli::OutputFormat;
use crate::commands::task_common::*;
use crate::commands::task_index::{TaskRecord, assign_canonical_ids, collect_agenda_records};
use crate::config::ResolvedConfig;
use crate::output::{Column, OutputContext};
use crate::workspace::Workspace;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::Serialize;

impl RowItem for AgendaItem {
    fn id(&self) -> usize {
        self.id
    }
    fn todo_state(&self) -> Option<&str> {
        self.todo_state.as_deref()
    }
    fn priority(&self) -> Option<char> {
        self.priority
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn heading_title(&self) -> &str {
        &self.heading_title
    }
    fn filetags(&self) -> &[String] {
        &self.filetags
    }
    fn heading_tags(&self) -> &[String] {
        &self.heading_tags
    }
    fn scheduled(&self) -> Option<&str> {
        self.scheduled.as_deref()
    }
    fn deadline(&self) -> Option<&str> {
        self.deadline.as_deref()
    }
    fn daily_file_date(&self) -> Option<&str> {
        self.daily_file_date.as_deref()
    }
    fn scheduled_date_str(&self) -> Option<&str> {
        self.scheduled_date.as_deref()
    }
    fn deadline_date_str(&self) -> Option<&str> {
        self.deadline_date.as_deref()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgendaItem {
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub has_agenda_tag: bool,
    pub is_daily_file: bool,
    pub daily_file_date: Option<String>,
    pub heading_title: String,
    pub heading_level: usize,
    pub line_number: usize,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub deadline_date: Option<String>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

pub struct AgendaOptions {
    pub state: Option<String>,
    pub tags: Option<String>,
    pub kind: Option<String>,
    pub prio: Option<String>,
    pub overdue: bool,
    pub upcoming: bool,
    pub date: Option<NaiveDate>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub today: bool,
    pub week: bool,
    pub line_sep: bool,
    pub columns: Vec<Column>,
}

impl From<TaskRecord> for AgendaItem {
    fn from(record: TaskRecord) -> Self {
        AgendaItem {
            id: record.id,
            uuid: record.uuid,
            title: record.title,
            path: record.path,
            filetags: record.filetags,
            has_agenda_tag: record.has_agenda_tag,
            is_daily_file: record.is_daily_file,
            daily_file_date: record.daily_file_date,
            heading_title: record.heading_title,
            heading_level: record.heading_level,
            line_number: record.line_number,
            todo_state: record.todo_state,
            priority: record.priority,
            scheduled: record.scheduled,
            scheduled_date: record.scheduled_date,
            deadline: record.deadline,
            deadline_date: record.deadline_date,
            is_overdue: record.is_overdue,
            heading_tags: record.heading_tags,
        }
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &AgendaOptions) -> Result<()> {
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;

    let today_date = Local::now().date_naive();

    let date_filter = opts
        .date
        .or(if opts.today { Some(today_date) } else { None });

    let week_cutoff = if opts.week {
        Some(today_date + chrono::Duration::days(7))
    } else {
        None
    };

    let valid_states = config.todo_states();
    let closed_states = config.closed_todo_states();
    let state_filters = parse_filters(opts.state.as_deref());
    let tags_filters = parse_filters(opts.tags.as_deref());
    let type_filters = parse_filters(opts.kind.as_deref());
    let mut records = collect_agenda_records(
        &workspace.corpus,
        &valid_states,
        &closed_states,
        today_date,
        &state_filters,
        &tags_filters,
        &type_filters,
    );

    if let Some(cutoff) = week_cutoff {
        records.retain(|item| {
            item.scheduled_date
                .as_deref()
                .or(item.deadline_date.as_deref())
                .or(item.daily_file_date.as_deref())
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .is_some_and(|d| d <= cutoff)
        });
    } else if let Some(filter_date) = date_filter {
        let filter_date = filter_date.format("%Y-%m-%d").to_string();
        records.retain(|item| {
            item.scheduled_date.as_deref() == Some(filter_date.as_str())
                || item.deadline_date.as_deref() == Some(filter_date.as_str())
                || (item.is_daily_file
                    && item.daily_file_date.as_deref() == Some(filter_date.as_str()))
        });
    }

    if opts.overdue {
        records.retain(|item| item.is_overdue);
    }

    assign_canonical_ids(config, graph, &mut records);
    let mut items: Vec<AgendaItem> = records.into_iter().map(AgendaItem::from).collect();

    if opts.upcoming {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        items.retain(|item| {
            let is_today = item.scheduled_date.as_deref() == Some(today_str.as_str())
                || item.deadline_date.as_deref() == Some(today_str.as_str())
                || (item.is_daily_file
                    && item.daily_file_date.as_deref() == Some(today_str.as_str()));
            !item.is_overdue && !is_today
        });
    }

    if let Some(ref prio) = opts.prio {
        if prio.is_empty() {
            items.retain(|item| item.priority.is_none());
        } else if let Some(p) = prio.chars().next() {
            let target = p.to_ascii_uppercase();
            items.retain(|item| item.priority == Some(target));
        }
    }

    let sort_fields: Vec<&str> = opts
        .sort
        .as_deref()
        .map(|s| s.split(',').map(|s| s.trim()).collect())
        .unwrap_or_else(|| vec!["date", "priority"]);
    sort_items(&mut items, &sort_fields);

    let total = items.len();

    if let Some(l) = opts.limit {
        items.truncate(l);
    }

    let flat = opts.today || opts.upcoming || opts.overdue;

    match ctx.format {
        OutputFormat::Text => {
            if flat {
                let sections = [("", items.as_slice())];
                let footer = format!("Total: {} planned item(s)", items.len());
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            } else {
                let today = Local::now().date_naive();
                let today_str = today.format("%Y-%m-%d").to_string();

                let mut overdue = Vec::new();
                let mut today_items = Vec::new();
                let mut upcoming = Vec::new();

                for item in &items {
                    if item.is_overdue {
                        overdue.push(item.clone());
                    } else if item.scheduled_date.as_deref() == Some(today_str.as_str())
                        || item.deadline_date.as_deref() == Some(today_str.as_str())
                        || (item.is_daily_file
                            && item.daily_file_date.as_deref() == Some(today_str.as_str()))
                    {
                        today_items.push(item.clone());
                    } else if item.scheduled_date.is_some()
                        || item.deadline_date.is_some()
                        || item.is_daily_file
                    {
                        upcoming.push(item.clone());
                    }
                }

                overdue.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
                today_items.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
                upcoming.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));

                let sections: [(&str, &[AgendaItem]); 3] = [
                    ("=== Overdue ===", &overdue),
                    ("=== Today ===", &today_items),
                    ("=== Upcoming ===", &upcoming),
                ];
                let footer = format!("Total: {} planned item(s)", items.len());
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            }
        }
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct AgendaOutput {
                total: usize,
                items: Vec<AgendaItem>,
            }
            ctx.print_json(&AgendaOutput { total, items })?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&items)?;
        }
    }

    Ok(())
}
