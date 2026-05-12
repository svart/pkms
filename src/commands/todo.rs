use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::OutputContext;
use crate::parser::{find_daily_file_date, strip_org_links};
use anyhow::Result;
use chrono::Local;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TodoItem {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub is_daily_file: bool,
    pub daily_file_date: Option<String>,
    pub heading_title: String,
    pub heading_level: usize,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub deadline_date: Option<String>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

fn extract_date(raw: Option<&String>) -> Option<String> {
    let raw = raw.as_ref()?;
    let parsed = parse_org_date(raw)?;
    Some(parsed.base_date.format("%Y-%m-%d").to_string())
}

fn is_overdue(raw: Option<&String>) -> bool {
    let raw = match raw {
        Some(r) => r,
        None => return false,
    };
    let parsed = match parse_org_date(raw) {
        Some(d) => d,
        None => return false,
    };
    let today = Local::now().date_naive();
    parsed.base_date < today
}

fn heading_is_eligible(heading: &crate::parser::Heading, valid_states: &[String]) -> bool {
    heading
        .todo_state
        .as_ref()
        .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)))
}

pub struct TodoOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub group: Option<String>,
    pub scope: Vec<String>,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &TodoOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    let valid_states = config.todo_states();

    let today_date = Local::now().date_naive();

    let mut items: Vec<TodoItem> = Vec::new();

    for result in &graph.results {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path).map(|d| d.format("%Y-%m-%d").to_string());

        for heading in &parsed.headings {
            if !heading_is_eligible(heading, &valid_states) {
                continue;
            }

            if !opts.include.is_empty() {
                let state_matches = heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| opts.include.iter().any(|is| is.eq_ignore_ascii_case(s)));
                if !state_matches {
                    continue;
                }
            }

            if !opts.exclude.is_empty()
                && let Some(ref todo_state) = heading.todo_state
                && opts
                    .exclude
                    .iter()
                    .any(|es| es.eq_ignore_ascii_case(todo_state))
            {
                continue;
            }

            let item_scheduled_date = extract_date(heading.scheduled.as_ref());
            let item_deadline_date = extract_date(heading.deadline.as_ref());
            let item_is_overdue =
                is_overdue(heading.deadline.as_ref()) || is_overdue(heading.scheduled.as_ref());

            let primary_uuid = parsed.uuids.first().cloned().unwrap_or_default();
            let note_title = strip_org_links(&parsed.title.clone().unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            }));

            items.push(TodoItem {
                uuid: primary_uuid,
                title: note_title,
                path: path.to_string_lossy().to_string(),
                filetags: parsed.filetags.clone(),
                is_daily_file: is_daily,
                daily_file_date: daily_date.clone(),
                heading_title: strip_org_links(&heading.title),
                heading_level: heading.level,
                todo_state: heading.todo_state.clone(),
                priority: heading.priority,
                scheduled: heading.scheduled.clone(),
                scheduled_date: item_scheduled_date,
                deadline: heading.deadline.clone(),
                deadline_date: item_deadline_date,
                is_overdue: item_is_overdue,
                heading_tags: heading.tags.clone(),
            });
        }
    }

    if !opts.scope.is_empty() {
        let db_root = config.resolved_db_root()?;
        let mut scope_paths: Vec<std::path::PathBuf> = Vec::new();
        for s in &opts.scope {
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
        let item_paths: std::collections::HashSet<String> = scope_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        items.retain(|item| item_paths.contains(&item.path));
    }

    let sort_field = opts.sort.as_deref().unwrap_or("priority");

    if let Some(group_field) = &opts.group {
        let mut groups: std::collections::BTreeMap<String, Vec<TodoItem>> =
            std::collections::BTreeMap::new();
        for item in items {
            let key = get_group_key(&item, group_field);
            groups.entry(key).or_default().push(item);
        }

        for group_items in groups.values_mut() {
            sort_items(group_items, sort_field);
            if let Some(l) = opts.limit {
                group_items.truncate(l);
            }
        }

        let total = groups.values().map(|g| g.len()).sum();

        match ctx.format {
            OutputFormat::Text => print_todo_text_grouped(&groups, group_field, total, &today_date),
            OutputFormat::Json => {
                ctx.print_json(&serde_json::json!({
                    "total": total,
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
        sort_items(&mut items, sort_field);

        let total = items.len();

        if let Some(l) = opts.limit {
            items.truncate(l);
        }

        match ctx.format {
            OutputFormat::Text => print_todo_text(&items, total, &today_date),
            OutputFormat::Json => {
                #[derive(Serialize)]
                struct TodoOutput {
                    total: usize,
                    items: Vec<TodoItem>,
                }
                ctx.print_json(&TodoOutput { total, items })?;
            }
            OutputFormat::Ndjson => {
                ctx.print_ndjson(&items)?;
            }
        }
    }

    Ok(())
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

fn sort_items(items: &mut [TodoItem], sort_field: &str) {
    match sort_field {
        "state" => {
            items.sort_by(|a, b| {
                a.todo_state
                    .as_deref()
                    .cmp(&b.todo_state.as_deref())
                    .then_with(|| a.title.cmp(&b.title))
            });
        }
        "file" => {
            items.sort_by(|a, b| a.title.cmp(&b.title));
        }
        _ => {
            items.sort_by(|a, b| {
                let a_priority = a.priority.map(priority_value).unwrap_or(3);
                let b_priority = b.priority.map(priority_value).unwrap_or(3);
                a_priority
                    .cmp(&b_priority)
                    .then_with(|| a.title.cmp(&b.title))
            });
        }
    }
}

fn priority_value(p: char) -> u8 {
    match p {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
    }
}

fn print_todo_text(items: &[TodoItem], total: usize, _today_date: &chrono::NaiveDate) {
    if items.is_empty() {
        println!("No TODO items found.");
        return;
    }

    let mut by_state: std::collections::BTreeMap<String, Vec<&TodoItem>> =
        std::collections::BTreeMap::new();
    for item in items {
        let state = item.todo_state.as_deref().unwrap_or("NONE").to_string();
        by_state.entry(state).or_default().push(item);
    }

    for (state, group) in &by_state {
        println!("=== {state} ({}) ===", group.len());
        for item in group {
            let prio = item
                .priority
                .map(|p| format!("[#{}] ", p))
                .unwrap_or_default();
            let date_info = item
                .scheduled_date
                .as_deref()
                .or(item.deadline_date.as_deref())
                .map(|d| format!(" ({})", d))
                .unwrap_or_default();
            println!(
                "  {}{}{}  \u{2014} {}{}",
                prio,
                item.heading_title,
                date_info,
                item.title,
                if item.is_daily_file { " [daily]" } else { "" }
            );
        }
        println!();
    }

    println!("Total: {total} TODO item(s)");
}

fn print_todo_text_grouped(
    groups: &std::collections::BTreeMap<String, Vec<TodoItem>>,
    group_field: &str,
    total: usize,
    _today_date: &chrono::NaiveDate,
) {
    if groups.is_empty() || groups.values().all(|g| g.is_empty()) {
        println!("No TODO items found.");
        return;
    }

    let show_note_title = group_field != "file";

    for (group_key, group) in groups {
        if group.is_empty() {
            continue;
        }
        println!("=== {group_key} ({}) ===", group.len());
        for item in group {
            let prio = item
                .priority
                .map(|p| format!("[#{}] ", p))
                .unwrap_or_default();
            let date_info = item
                .scheduled_date
                .as_deref()
                .or(item.deadline_date.as_deref())
                .map(|d| format!(" ({})", d))
                .unwrap_or_default();
            if show_note_title {
                println!(
                    "  {}{}{}  \u{2014} {}{}",
                    prio,
                    item.heading_title,
                    date_info,
                    item.title,
                    if item.is_daily_file { " [daily]" } else { "" }
                );
            } else {
                println!(
                    "  {}{}{}{}",
                    prio,
                    item.heading_title,
                    date_info,
                    if item.is_daily_file { " [daily]" } else { "" }
                );
            }
        }
        println!();
    }

    println!("Total: {total} TODO item(s)");
}
