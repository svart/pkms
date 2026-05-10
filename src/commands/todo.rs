use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::OutputContext;
use crate::parser::find_daily_file_date;
use anyhow::Result;
use chrono::Local;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct TodoItem {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub has_agenda_tag: bool,
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

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: &Config,
    ctx: &OutputContext,
    missing_agenda: bool,
    include: Option<&str>,
    exclude: Option<&str>,
    sort: Option<&str>,
    limit: Option<usize>,
    db_cli: Option<&Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();
    let results = Graph::scan(&db_root, &ignore)?;

    let valid_states = config.todo_states();

    let today_date = Local::now().date_naive();

    let include_set: Option<Vec<String>> =
        include.map(|s| s.split(',').map(|s| s.trim().to_string()).collect());
    let exclude_set: Option<Vec<String>> =
        exclude.map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

    let mut items: Vec<TodoItem> = Vec::new();

    for result in &results {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let has_agenda = parsed.filetags.iter().any(|t| t == "agenda");
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path).map(|d| d.format("%Y-%m-%d").to_string());

        for heading in &parsed.headings {
            if !heading_is_eligible(heading, &valid_states) {
                continue;
            }

            if let Some(ref incl) = include_set {
                let state_matches = heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| incl.iter().any(|is| is.eq_ignore_ascii_case(s)));
                if !state_matches {
                    continue;
                }
            }

            if let Some(ref excl) = exclude_set
                && let Some(ref todo_state) = heading.todo_state
                && excl.iter().any(|es| es.eq_ignore_ascii_case(todo_state))
            {
                continue;
            }

            if missing_agenda && has_agenda {
                continue;
            }

            let item_scheduled_date = extract_date(heading.scheduled.as_ref());
            let item_deadline_date = extract_date(heading.deadline.as_ref());
            let item_is_overdue = is_overdue(heading.deadline.as_ref());

            let primary_uuid = parsed.uuids.first().cloned().unwrap_or_default();
            let note_title = parsed.title.clone().unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });

            items.push(TodoItem {
                uuid: primary_uuid,
                title: note_title,
                path: path.to_string_lossy().to_string(),
                filetags: parsed.filetags.clone(),
                has_agenda_tag: has_agenda,
                is_daily_file: is_daily,
                daily_file_date: daily_date.clone(),
                heading_title: heading.title.clone(),
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

    let sort_field = sort.unwrap_or("priority");
    sort_items(&mut items, sort_field);

    let total = items.len();

    if let Some(l) = limit {
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

    Ok(())
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

    let missing_agenda_items: Vec<&TodoItem> = items.iter().filter(|i| !i.has_agenda_tag).collect();

    if !missing_agenda_items.is_empty() {
        println!("=== Missing :agenda: tag ===");
        let mut by_file: std::collections::BTreeMap<&str, (usize, &str)> =
            std::collections::BTreeMap::new();
        for item in &missing_agenda_items {
            let entry = by_file
                .entry(item.title.as_str())
                .or_insert_with(|| (0, item.uuid.as_str()));
            entry.0 += 1;
        }
        for (title, (count, uuid)) in &by_file {
            println!("  {title} ({uuid})  {count} TODOs, missing :agenda:");
        }
        println!();
    }

    println!("Total: {total} TODO item(s)");
}
