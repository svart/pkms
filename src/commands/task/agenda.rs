use crate::cli::OutputFormat;
use crate::commands::task_common::*;
use crate::commands::task_index::{TaskRecord, assign_canonical_ids, collect_agenda_records_on};
use crate::config::ResolvedConfig;
use crate::output::{Column, OutputContext};
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::parse_text_filters;
use crate::tasks::model::TaskItem;
use crate::tasks::pkms::record_to_task_item;
use crate::workspace::Workspace;
use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::BTreeMap;

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
    pub days: Option<i64>,
    pub today: bool,
    pub week: bool,
    pub line_sep: bool,
    pub columns: Vec<Column>,
}

pub fn run_with_clock(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    opts: &AgendaOptions,
    clock: TaskClock,
) -> Result<()> {
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;
    let today_date = clock.today;

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
    let state_filters = parse_text_filters(opts.state.as_deref());
    let tags_filters = parse_text_filters(opts.tags.as_deref());
    let type_filters = parse_text_filters(opts.kind.as_deref());
    let mut records = collect_agenda_records_on(
        &workspace.corpus,
        &valid_states,
        &closed_states,
        clock,
        &state_filters,
        &tags_filters,
        &type_filters,
    );

    if let Some(cutoff) = week_cutoff {
        records.retain(|item| {
            item.effective_date()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .is_some_and(|d| d <= cutoff)
        });
    } else if let Some(filter_date) = date_filter {
        let filter_date = filter_date.format("%Y-%m-%d").to_string();
        records.retain(|item| item.has_effective_date(filter_date.as_str()));
    }

    if let Some(days) = opts.days {
        records.retain(|item| record_in_agenda_window(item, today_date, days));
    }

    if opts.overdue {
        records.retain(|item| item.is_overdue);
    }

    assign_canonical_ids(config, graph, &mut records);
    let mut items = records;

    if opts.upcoming {
        let today_str = today_date.format("%Y-%m-%d").to_string();
        items.retain(|item| {
            let is_today = item.has_effective_date(today_str.as_str());
            !item.is_overdue && !is_today
        });
    }

    if let Some(ref prio) = opts.prio {
        if prio.is_empty() {
            items.retain(|item| item.priority.is_none());
        } else {
            let targets = crate::tasks::filter::priority_filter_targets(prio);
            items.retain(|item| {
                crate::tasks::filter::priority_matches_target(item.priority, &targets)
            });
        }
    }

    let sort_fields = parse_task_sort_fields(opts.sort.as_deref().unwrap_or("date,priority"))?;
    sort_items(&mut items, &sort_fields);

    let total = apply_limit(&mut items, opts.limit);

    let flat = opts.today || opts.upcoming || opts.overdue;

    match ctx.format {
        OutputFormat::Text => {
            if flat {
                let sections = [("", items.as_slice())];
                let footer = format!("Total: {} planned item(s)", items.len());
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            } else if let Some(days) = opts.days {
                print_windowed_agenda_task_table(
                    &items,
                    days,
                    today_date,
                    &opts.columns,
                    opts.line_sep,
                );
            } else {
                let today_str = today_date.format("%Y-%m-%d").to_string();

                let mut overdue = Vec::new();
                let mut today_items = Vec::new();
                let mut upcoming = Vec::new();

                for item in &items {
                    if item.is_overdue {
                        overdue.push(item.clone());
                    } else if item.has_effective_date(today_str.as_str()) {
                        today_items.push(item.clone());
                    } else if item.effective_date().is_some() {
                        upcoming.push(item.clone());
                    }
                }

                overdue.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
                today_items.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
                upcoming.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));

                let sections: [(&str, &[TaskRecord]); 3] = [
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
                items: Vec<TaskItem>,
            }
            ctx.print_json(&AgendaOutput {
                total,
                items: items
                    .into_iter()
                    .map(|record| record_to_task_item(config, record))
                    .collect(),
            })?;
        }
        OutputFormat::Ndjson => {
            let items = items
                .into_iter()
                .map(|record| record_to_task_item(config, record))
                .collect::<Vec<_>>();
            ctx.print_ndjson(&items)?;
        }
    }

    Ok(())
}

fn record_in_agenda_window(item: &TaskRecord, today: NaiveDate, days: i64) -> bool {
    item.is_overdue
        || record_date(item).is_some_and(|date| date_in_agenda_window(date, today, days))
}

fn record_date(item: &TaskRecord) -> Option<NaiveDate> {
    item.effective_date()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

fn print_windowed_agenda_task_table(
    items: &[TaskRecord],
    days: i64,
    today: NaiveDate,
    columns: &[Column],
    line_sep: bool,
) {
    let mut overdue = Vec::new();
    let mut daily_items: BTreeMap<i64, Vec<TaskRecord>> = BTreeMap::new();

    for item in items {
        if item.is_overdue {
            overdue.push(item.clone());
        } else if let Some(date) = record_date(item) {
            let offset = (date - today).num_days();
            if (0..days).contains(&offset) {
                daily_items.entry(offset).or_default().push(item.clone());
            }
        }
    }

    overdue.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
    for items in daily_items.values_mut() {
        items.sort_by(|a, b| a.effective_date().cmp(&b.effective_date()));
    }

    let mut labels = vec!["=== Overdue ===".to_string()];
    let mut groups = vec![overdue];
    for (offset, items) in daily_items {
        let date = today + chrono::Duration::days(offset);
        labels.push(agenda_day_section_label(today, date));
        groups.push(items);
    }
    let sections: Vec<(&str, &[TaskRecord])> = labels
        .iter()
        .zip(groups.iter())
        .map(|(label, items)| (label.as_str(), items.as_slice()))
        .collect();
    let footer = format!("Total: {} planned item(s)", items.len());
    print_table(&sections, columns, line_sep, &footer);
}
