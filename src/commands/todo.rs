use crate::cli::OutputFormat;
use crate::commands::task_common::*;
use crate::commands::task_index::{TaskRecord, assign_canonical_ids, collect_todo_records};
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::{Column, OutputContext};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

impl RowItem for TodoItem {
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
pub struct TodoItem {
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
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

impl From<TaskRecord> for TodoItem {
    fn from(record: TaskRecord) -> Self {
        TodoItem {
            id: record.id,
            uuid: record.uuid,
            title: record.title,
            path: record.path,
            filetags: record.filetags,
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

fn item_datetimes(item: &TodoItem) -> Vec<NaiveDateTime> {
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

fn resolve_scope_paths(
    graph: &Graph,
    scope: &[String],
    db_root: &Path,
) -> std::collections::HashSet<String> {
    let mut scope_paths: Vec<std::path::PathBuf> = Vec::new();
    for s in scope {
        if let Some(node) = graph.find_node(s) {
            scope_paths.push(node.path.clone());
            continue;
        }
        let expanded = if let Some(rest) = s.strip_prefix("~/") {
            dirs::home_dir().map(|h| h.join(rest))
        } else {
            None
        };
        let mut matched = false;
        for candidate in [Some(std::path::Path::new(s)), expanded.as_deref()]
            .into_iter()
            .flatten()
        {
            for p in [candidate.to_path_buf()]
                .into_iter()
                .chain(candidate.canonicalize().ok())
            {
                if graph.results.iter().any(|r| r.path == p) {
                    scope_paths.push(p);
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
        let joined = db_root.join(s);
        for p in [joined.clone()]
            .into_iter()
            .chain(joined.canonicalize().ok())
        {
            if graph.results.iter().any(|r| r.path == p) {
                scope_paths.push(p);
                break;
            }
        }
    }
    scope_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect()
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &TodoOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    let valid_states = config.todo_states();
    let state_filters = parse_filters(opts.state.as_deref());
    let tags_filters = parse_filters(opts.tags.as_deref());
    let type_filters = parse_filters(opts.kind.as_deref());

    let mut records = collect_todo_records(
        &graph,
        &valid_states,
        &state_filters,
        &tags_filters,
        &type_filters,
    );
    assign_canonical_ids(config, &graph, &mut records);
    let mut items: Vec<TodoItem> = records.into_iter().map(TodoItem::from).collect();

    if !opts.scope.is_empty() {
        let db_root = config.resolved_db_root()?;
        let item_paths = resolve_scope_paths(&graph, &opts.scope, db_root);
        items.retain(|item| item_paths.contains(&item.path));
    }

    if let Some(ref prio) = opts.prio {
        if prio.is_empty() {
            items.retain(|item| item.priority.is_none());
        } else if let Some(p) = prio.chars().next() {
            let target = p.to_ascii_uppercase();
            items.retain(|item| item.priority == Some(target));
        }
    }

    if let Some(ref after_dt) = opts.after {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt >= after_dt));
    }

    if let Some(ref before_dt) = opts.before {
        items.retain(|item| item_datetimes(item).iter().any(|dt| dt <= before_dt));
    }

    let sort_fields: Vec<&str> = opts
        .sort
        .as_deref()
        .map(|s| s.split(',').map(|s| s.trim()).collect())
        .unwrap_or_else(|| vec!["priority"]);

    if let Some(group_field) = &opts.group {
        let mut groups: BTreeMap<String, Vec<TodoItem>> = BTreeMap::new();
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
            .map(|(k, v)| format!("=== {k} ({}) ===", v.len()))
            .collect();

        match ctx.format {
            OutputFormat::Text => {
                let sections: Vec<(&str, &[TodoItem])> = section_labels
                    .iter()
                    .zip(groups.values())
                    .map(|(l, v)| (l.as_str(), v.as_slice()))
                    .collect();
                print_table(&sections, &opts.columns, opts.line_sep, &footer);
            }
            OutputFormat::Json => {
                ctx.print_json(&serde_json::json!({
                    "total": shown,
                    "group_field": group_field,
                    "groups": groups,
                }))?;
            }
            OutputFormat::Ndjson => {
                for (group_key, group_items) in &groups {
                    for item in group_items {
                        let mut json_item = serde_json::to_value(item)?;
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

        let total_before_limit = items.len();

        if let Some(l) = opts.limit {
            items.truncate(l);
        }

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
                    items: Vec<TodoItem>,
                }
                ctx.print_json(&TodoOutput {
                    total: shown,
                    items,
                })?;
            }
            OutputFormat::Ndjson => {
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

fn get_group_key(item: &TodoItem, group_field: &str) -> String {
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
