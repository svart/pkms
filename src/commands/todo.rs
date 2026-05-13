use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::{OutputContext, adaptive_column_widths};
use crate::parser::{find_daily_file_date, strip_org_links};
use anyhow::Result;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::Serialize;
use std::collections::BTreeMap;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Span, Width};

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

fn combine_tags(filetags: &[String], heading_tags: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for tag in filetags.iter().chain(heading_tags.iter()) {
        if seen.insert(tag.clone()) {
            result.push(tag.clone());
        }
    }
    result.join(", ")
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
    let compare_date = parsed.base_date_end.unwrap_or(parsed.base_date);
    if compare_date < today {
        return true;
    }
    if compare_date == today
        && let Some(et) = parsed.time_end
    {
        let now = Local::now().time();
        return now > et;
    }
    false
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
    pub after: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub prio: Option<String>,
    pub line_sep: bool,
}

fn item_datetimes(item: &TodoItem) -> Vec<NaiveDateTime> {
    let mut result = Vec::new();
    if let Some(ref raw) = item.scheduled
        && let Some(parsed) = parse_org_date(raw)
    {
        let time = parsed
            .time
            .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        result.push(parsed.base_date.and_time(time));
    }
    if let Some(ref raw) = item.deadline
        && let Some(parsed) = parse_org_date(raw)
    {
        let time = parsed
            .time
            .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        result.push(parsed.base_date.and_time(time));
    }
    if let Some(ref d) = item.daily_file_date
        && let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d")
    {
        result.push(date.and_hms_opt(0, 0, 0).unwrap());
    }
    result
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

    let sort_field = opts.sort.as_deref().unwrap_or("priority");

    if let Some(group_field) = &opts.group {
        let mut groups: BTreeMap<String, Vec<TodoItem>> = BTreeMap::new();
        for item in items {
            let key = get_group_key(&item, group_field);
            groups.entry(key).or_default().push(item);
        }

        let total_before_limit: usize = groups.values().map(|g| g.len()).sum();

        for group_items in groups.values_mut() {
            sort_items(group_items, sort_field);
            if let Some(l) = opts.limit {
                group_items.truncate(l);
            }
        }

        let shown: usize = groups.values().map(|g| g.len()).sum();

        match ctx.format {
            OutputFormat::Text => print_todo_text_grouped(
                &groups,
                group_field,
                shown,
                total_before_limit,
                &today_date,
                opts.line_sep,
            ),
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
        sort_items(&mut items, sort_field);

        let total_before_limit = items.len();

        if let Some(l) = opts.limit {
            items.truncate(l);
        }

        let shown = items.len();

        match ctx.format {
            OutputFormat::Text => print_todo_text(
                &items,
                shown,
                total_before_limit,
                &today_date,
                opts.line_sep,
            ),
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

fn format_display_datetime(raw: &str) -> String {
    let parsed = parse_org_date(raw);
    match parsed {
        Some(d) => {
            let date_str = d.base_date.format("%Y-%m-%d %a").to_string();
            let start_line = if let Some(t) = d.time {
                format!("{} {:02}:{:02}", date_str, t.hour(), t.minute())
            } else {
                date_str.clone()
            };
            if let Some(end_date) = d.base_date_end {
                let end_date_str = end_date.format("%Y-%m-%d %a").to_string();
                let end_line = if let Some(et) = d.time_end {
                    format!("{} {:02}:{:02}", end_date_str, et.hour(), et.minute())
                } else {
                    end_date_str
                };
                return format!("{}\n{}", start_line, end_line);
            }
            if let Some(et) = d.time_end {
                let padding = " ".repeat(date_str.len() + 1);
                return format!(
                    "{}\n{}{:02}:{:02}",
                    start_line,
                    padding,
                    et.hour(),
                    et.minute()
                );
            }
            start_line
        }
        None => raw.to_string(),
    }
}

fn format_todo_rows(item: &TodoItem) -> Vec<[String; 7]> {
    let state = item.todo_state.as_deref().unwrap_or("").to_string();
    let prio = item
        .priority
        .map(|p| format!("[#{}]", p))
        .unwrap_or_default();
    let title = item.title.clone();
    let heading = item.heading_title.clone();
    let tags = combine_tags(&item.filetags, &item.heading_tags);
    let has_both = item.scheduled.is_some() && item.deadline.is_some();
    let mut rows = Vec::new();

    if let Some(ref s) = item.scheduled {
        rows.push([
            format_display_datetime(s),
            state.clone(),
            "SCHED".to_string(),
            prio.clone(),
            tags.clone(),
            title.clone(),
            heading.clone(),
        ]);
    }

    if let Some(ref d) = item.deadline {
        rows.push([
            format_display_datetime(d),
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
                title.clone()
            },
            if has_both {
                String::new()
            } else {
                heading.clone()
            },
        ]);
    }

    if rows.is_empty()
        && let Some(ref dfd) = item.daily_file_date
    {
        rows.push([
            dfd.clone(),
            state,
            String::new(),
            prio,
            tags,
            title,
            heading,
        ]);
    }

    rows
}

fn push_todo_rows(builder: &mut Builder, item: &TodoItem) -> usize {
    let rows = format_todo_rows(item);
    let count = rows.len();
    for row in rows {
        builder.push_record(row);
    }
    count
}

fn print_todo_text(
    items: &[TodoItem],
    shown: usize,
    total: usize,
    _today_date: &chrono::NaiveDate,
    line_sep: bool,
) {
    if items.is_empty() {
        println!("No TODO items found.");
        return;
    }

    let headers = ["Date", "State", "Type", "Prio", "Tags", "Note", "Heading"];
    let mut max_widths: [usize; 7] = headers.map(|h| h.len());
    let mut total_rows = 1;
    for item in items {
        for row in format_todo_rows(item) {
            for (i, col) in row.iter().enumerate() {
                let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                max_widths[i] = max_widths[i].max(line_w);
            }
        }
        total_rows += format_todo_rows(item).len();
    }

    let fixed_sum = max_widths[0] + max_widths[1] + max_widths[2] + max_widths[3];
    let wrap = adaptive_column_widths(max_widths[4], max_widths[5], fixed_sum);

    let mut builder = Builder::new();
    builder.push_record(headers);

    for item in items {
        push_todo_rows(&mut builder, item);
    }

    let mut table = builder.build();
    table.with(Style::blank());
    table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    if line_sep && total_rows > 2 {
        for i in 2..total_rows {
            table.with(Modify::new(Rows::one(i)).with(Border::new().top('─')));
        }
    }
    if let Some((tags_w, note_w, heading_w)) = wrap {
        table.with(Modify::new(Columns::new(4..5)).with(Width::wrap(tags_w).keep_words(true)));
        table.with(Modify::new(Columns::new(5..6)).with(Width::wrap(note_w).keep_words(true)));
        table.with(Modify::new(Columns::new(6..7)).with(Width::wrap(heading_w).keep_words(true)));
    }
    println!("{}", table);
    println!();
    if shown < total {
        println!("Shown: {shown}, Total: {total} TODO item(s)");
    } else {
        println!("Total: {total} TODO item(s)");
    }
}

fn print_todo_text_grouped(
    groups: &BTreeMap<String, Vec<TodoItem>>,
    _group_field: &str,
    shown: usize,
    total: usize,
    _today_date: &chrono::NaiveDate,
    line_sep: bool,
) {
    if groups.is_empty() || groups.values().all(|g| g.is_empty()) {
        println!("No TODO items found.");
        return;
    }

    let headers = ["Date", "State", "Type", "Prio", "Tags", "Note", "Heading"];
    let mut max_widths: [usize; 7] = headers.map(|h| h.len());
    for group in groups.values() {
        for item in group {
            for row in format_todo_rows(item) {
                for (i, col) in row.iter().enumerate() {
                    let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                    max_widths[i] = max_widths[i].max(line_w);
                }
            }
        }
    }

    let fixed_sum = max_widths[0] + max_widths[1] + max_widths[2] + max_widths[3];
    let wrap = adaptive_column_widths(max_widths[4], max_widths[5], fixed_sum);

    let mut builder = Builder::new();
    builder.push_record(headers);

    let mut section_rows: Vec<usize> = Vec::new();
    let mut row = 1;
    for (group_key, group) in groups {
        if group.is_empty() {
            continue;
        }
        if row > 1 {
            builder.push_record(["", "", "", "", "", "", ""]);
            row += 1;
        }
        let label = format!("=== {group_key} ({}) ===", group.len());
        builder.push_record([label.as_str(), "", "", "", "", "", ""]);
        section_rows.push(row);
        row += 1;
        for item in group {
            row += push_todo_rows(&mut builder, item);
        }
    }

    let mut table = builder.build();
    for &sec_row in &section_rows {
        table.with(Modify::new((sec_row, 0)).with(Span::column(7)));
    }
    table.with(Style::blank());
    table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    if line_sep && row > 2 {
        for i in 2..row {
            table.with(Modify::new(Rows::one(i)).with(Border::new().top('─')));
        }
    }
    if let Some((tags_w, note_w, heading_w)) = wrap {
        table.with(Modify::new(Columns::new(4..5)).with(Width::wrap(tags_w).keep_words(true)));
        table.with(Modify::new(Columns::new(5..6)).with(Width::wrap(note_w).keep_words(true)));
        table.with(Modify::new(Columns::new(6..7)).with(Width::wrap(heading_w).keep_words(true)));
    }
    println!("{}", table);
    println!();
    if shown < total {
        println!("Shown: {shown}, Total: {total} TODO item(s)");
    } else {
        println!("Total: {total} TODO item(s)");
    }
}
