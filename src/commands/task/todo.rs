use crate::cli::OutputFormat;
use crate::commands::task_common::*;
use crate::commands::task_index::{TaskRecord, assign_canonical_ids, collect_todo_records_on};
use crate::config::ResolvedConfig;
use crate::org_date::parse_org_date;
use crate::output::{Column, OutputContext};
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::parse_text_filters;
use crate::tasks::model::TaskItem;
use crate::tasks::pkms::record_to_task_item;
use crate::tasks::scope::ResolvedScope;
use crate::workspace::Workspace;
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use std::collections::BTreeMap;

pub struct TodoOptions {
    pub state: Option<String>,
    pub tags: Option<String>,
    pub kind: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub group: Option<String>,
    pub scope: Vec<String>,
    pub after: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub prio: Option<String>,
    pub line_sep: bool,
    pub columns: Vec<Column>,
}

fn item_datetimes(item: &TaskRecord) -> Vec<NaiveDateTime> {
    let mut result = Vec::new();
    if let Some(ref raw) = item.scheduled
        && let Some(parsed) = parse_org_date(raw)
    {
        let time = parsed
            .time
            .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid"));
        result.push(parsed.base_date.and_time(time));
    }
    if let Some(ref raw) = item.deadline
        && let Some(parsed) = parse_org_date(raw)
    {
        let time = parsed
            .time
            .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid"));
        result.push(parsed.base_date.and_time(time));
    }
    if let Some(ref d) = item.daily_file_date
        && let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d")
    {
        result.push(date.and_hms_opt(0, 0, 0).expect("midnight is valid"));
    }
    result
}

pub fn run_on(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    opts: &TodoOptions,
    clock: TaskClock,
) -> Result<()> {
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;

    let valid_states = config.todo_states();
    let state_filters = parse_text_filters(opts.state.as_deref());
    let tags_filters = parse_text_filters(opts.tags.as_deref());
    let type_filters = parse_text_filters(opts.kind.as_deref());

    let mut records = collect_todo_records_on(
        &workspace.corpus,
        &valid_states,
        &state_filters,
        &tags_filters,
        &type_filters,
        clock,
    );
    assign_canonical_ids(config, graph, &mut records);
    let mut items = records;

    if !opts.scope.is_empty() {
        let db_root = config.resolved_db_root();
        let scope = ResolvedScope::resolve(graph, db_root, &opts.scope);
        items.retain(|item| scope.matches_path(&item.path));
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

    if let Some(ref after_dt) = opts.after {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt >= after_dt));
    }

    if let Some(ref before_dt) = opts.before {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt <= before_dt));
    }

    let sort_fields = parse_task_sort_fields(opts.sort.as_deref().unwrap_or("priority"))?;

    if let Some(group_field) = &opts.group {
        validate_task_group_field(group_field)?;
        let mut groups: BTreeMap<String, Vec<TaskRecord>> = BTreeMap::new();
        for item in items {
            let key = get_group_key(&item, group_field);
            groups.entry(key).or_default().push(item);
        }

        let total_before_limit: usize = groups.values().map(|g| g.len()).sum();

        for group_items in groups.values_mut() {
            sort_items(group_items, &sort_fields);
            if let Some(l) = opts.limit {
                group_items.truncate(l);
            }
        }

        let shown: usize = groups.values().map(|g| g.len()).sum();
        let footer = format_footer(shown, total_before_limit, "TODO");
        let section_labels: Vec<String> = groups
            .iter()
            .map(|(k, v)| format!("{k} ({})", v.len()))
            .collect();

        match ctx.format {
            OutputFormat::Text => {
                let sections: Vec<(&str, &[TaskRecord])> = section_labels
                    .iter()
                    .zip(groups.values())
                    .map(|(l, v)| (l.as_str(), v.as_slice()))
                    .collect();
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            }
            OutputFormat::Json => {
                let groups: BTreeMap<String, Vec<TaskItem>> = groups
                    .into_iter()
                    .map(|(key, items)| {
                        (
                            key,
                            items
                                .into_iter()
                                .map(|record| record_to_task_item(config, record))
                                .collect(),
                        )
                    })
                    .collect();
                ctx.print_json(&serde_json::json!({
                    "total": shown,
                    "group_field": group_field,
                    "groups": groups,
                }))?;
            }
            OutputFormat::Ndjson => {
                for (group_key, group_items) in groups {
                    for item in group_items {
                        let mut json_item =
                            serde_json::to_value(record_to_task_item(config, item))?;
                        json_item.as_object_mut().unwrap().insert(
                            "group".to_string(),
                            serde_json::Value::String(group_key.clone()),
                        );
                        println!("{}", serde_json::to_string(&json_item)?);
                    }
                }
            }
        }
    } else {
        sort_items(&mut items, &sort_fields);

        let total_before_limit = apply_limit(&mut items, opts.limit);
        let shown = items.len();
        let footer = format_footer(shown, total_before_limit, "TODO");

        match ctx.format {
            OutputFormat::Text => {
                let sections = [("", items.as_slice())];
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            }
            OutputFormat::Json => {
                #[derive(Serialize)]
                struct TodoOutput {
                    total: usize,
                    items: Vec<TaskItem>,
                }
                ctx.print_json(&TodoOutput {
                    total: shown,
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
    }

    Ok(())
}

fn format_footer(shown: usize, total: usize, label: &str) -> String {
    if shown < total {
        format!("Shown: {shown}, Total: {total} {label} item(s)")
    } else {
        format!("Total: {total} {label} item(s)")
    }
}

fn get_group_key(item: &TaskRecord, group_field: &str) -> String {
    match group_field {
        "state" => item.todo_state.as_deref().unwrap_or("NONE").to_string(),
        "file" => item.title.clone(),
        "priority" => match item.priority {
            Some('A') => "Priority A".to_string(),
            Some('B') => "Priority B".to_string(),
            Some('C') => "Priority C".to_string(),
            _ => "No Priority".to_string(),
        },
        _ => "Unknown".to_string(),
    }
}
