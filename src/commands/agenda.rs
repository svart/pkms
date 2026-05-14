use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::{ALL_COLUMNS, Column, OutputContext, adaptive_column_widths};
use crate::parser::{find_daily_file_date, strip_org_links};
use anyhow::Result;
use chrono::{Local, NaiveDate, Timelike};
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Object, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Span, Width};

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

enum Filter {
    Include(String),
    Exclude(String),
}

fn parse_filters(s: Option<&str>) -> Vec<Filter> {
    match s {
        Some(s) => s
            .split(',')
            .map(|s| {
                let s = s.trim().to_string();
                if let Some(rest) = s.strip_prefix('!') {
                    Filter::Exclude(rest.to_string())
                } else {
                    Filter::Include(s)
                }
            })
            .collect(),
        None => Vec::new(),
    }
}

fn apply_state_filter(state: Option<&str>, filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                if !state.is_some_and(|s| s.eq_ignore_ascii_case(v)) {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                if state.is_some_and(|s| s.eq_ignore_ascii_case(v)) {
                    return false;
                }
            }
        }
    }
    true
}

fn apply_tags_filter(tags: &[String], filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                if !tags.iter().any(|t| t == v) {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                if tags.iter().any(|t| t == v) {
                    return false;
                }
            }
        }
    }
    true
}

fn apply_type_filter(has_scheduled: bool, has_deadline: bool, filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::Include(v) => {
                let cond = match v.as_str() {
                    "SCHED" => has_scheduled,
                    "DEADL" => has_deadline,
                    _ => false,
                };
                if !cond {
                    return false;
                }
            }
            Filter::Exclude(v) => {
                let cond = match v.as_str() {
                    "SCHED" => has_scheduled,
                    "DEADL" => has_deadline,
                    _ => false,
                };
                if cond {
                    return false;
                }
            }
        }
    }
    true
}

fn heading_is_eligible(
    heading: &crate::parser::Heading,
    is_daily: bool,
    valid_states: &[String],
) -> bool {
    heading.scheduled.is_some()
        || heading.deadline.is_some()
        || (is_daily
            && heading
                .todo_state
                .as_ref()
                .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s))))
}

pub struct AgendaOptions {
    pub state: Option<String>,
    pub tags: Option<String>,
    pub type_: Option<String>,
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
    pub open: Option<usize>,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &AgendaOptions) -> Result<()> {
    let graph = Graph::load(config)?;

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
    let type_filters = parse_filters(opts.type_.as_deref());
    let mut items: Vec<AgendaItem> = Vec::new();

    for result in &graph.results {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let has_agenda = parsed.filetags.iter().any(|t| t == "agenda");
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path).map(|d| d.format("%Y-%m-%d").to_string());

        for heading in &parsed.headings {
            if !heading_is_eligible(heading, is_daily, &valid_states) {
                continue;
            }

            if !closed_states.is_empty()
                && let Some(ref todo_state) = heading.todo_state
                && closed_states
                    .iter()
                    .any(|cs| cs.eq_ignore_ascii_case(todo_state))
            {
                continue;
            }

            if !apply_state_filter(heading.todo_state.as_deref(), &state_filters) {
                continue;
            }

            let combined_tags: Vec<String> = {
                let mut seen = std::collections::HashSet::new();
                let mut result = Vec::new();
                for tag in parsed.filetags.iter().chain(heading.tags.iter()) {
                    if seen.insert(tag.clone()) {
                        result.push(tag.clone());
                    }
                }
                result
            };

            if !apply_tags_filter(&combined_tags, &tags_filters) {
                continue;
            }

            if !apply_type_filter(
                heading.scheduled.is_some(),
                heading.deadline.is_some(),
                &type_filters,
            ) {
                continue;
            }

            let item_scheduled_date = extract_date(heading.scheduled.as_ref());
            let item_deadline_date = extract_date(heading.deadline.as_ref());
            let item_is_overdue = if heading.scheduled.is_some() || heading.deadline.is_some() {
                is_overdue(heading.deadline.as_ref()) || is_overdue(heading.scheduled.as_ref())
            } else if is_daily {
                daily_date.as_deref().is_some_and(|d| {
                    NaiveDate::parse_from_str(d, "%Y-%m-%d")
                        .ok()
                        .is_some_and(|dt| dt < today_date)
                })
            } else {
                false
            };

            if let Some(cutoff) = week_cutoff {
                let item_date = item_scheduled_date
                    .as_deref()
                    .or(item_deadline_date.as_deref())
                    .or(daily_date.as_deref())
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
                let matches = item_date.is_some_and(|d| d <= cutoff);
                if !matches {
                    continue;
                }
            } else if let Some(filter_date) = date_filter {
                let matches = item_scheduled_date.as_deref()
                    == Some(&filter_date.format("%Y-%m-%d").to_string())
                    || item_deadline_date.as_deref()
                        == Some(&filter_date.format("%Y-%m-%d").to_string())
                    || (is_daily
                        && daily_date.as_deref()
                            == Some(&filter_date.format("%Y-%m-%d").to_string()));
                if !matches {
                    continue;
                }
            }

            if opts.overdue && !item_is_overdue {
                continue;
            }

            let primary_uuid = parsed.uuids.first().cloned().unwrap_or_default();
            let note_title = strip_org_links(&parsed.title.clone().unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            }));

            items.push(AgendaItem {
                id: 0,
                uuid: primary_uuid,
                title: note_title,
                path: path.to_string_lossy().to_string(),
                filetags: parsed.filetags.clone(),
                has_agenda_tag: has_agenda,
                is_daily_file: is_daily,
                daily_file_date: daily_date.clone(),
                heading_title: strip_org_links(&heading.title),
                heading_level: heading.level,
                line_number: heading.line_number,
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

    assign_canonical_ids(&mut items);

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

    if let Some(open_id) = opts.open {
        let target = items.iter().find(|item| item.id == open_id);
        match target {
            Some(item) => {
                let path = &item.path;
                let line = item.line_number;
                println!(
                    "Opening #{}: {} / {}",
                    item.id, item.title, item.heading_title
                );
                let status = std::process::Command::new("emacsclient")
                    .args(["-n", &format!("+{line}"), path])
                    .status();
                match status {
                    Ok(s) if s.success() => {}
                    Ok(s) => eprintln!("emacsclient exited with error: {s}"),
                    Err(e) => eprintln!("Failed to run emacsclient: {e}"),
                }
                return Ok(());
            }
            None => {
                anyhow::bail!("No task with ID {open_id} matching current filters");
            }
        }
    }

    let sort_fields: Vec<&str> = opts
        .sort
        .as_deref()
        .map(|s| s.split(',').map(|s| s.trim()).collect())
        .unwrap_or_else(|| vec!["priority"]);
    sort_items(&mut items, &sort_fields);

    let total = items.len();

    if let Some(l) = opts.limit {
        items.truncate(l);
    }

    match ctx.format {
        OutputFormat::Text => print_agenda_text(
            &items,
            opts.today || opts.upcoming || opts.overdue,
            opts.line_sep,
            &opts.columns,
        ),
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

fn sort_items(items: &mut [AgendaItem], sort_fields: &[&str]) {
    items.sort_by(|a, b| {
        for field in sort_fields {
            let ord = match *field {
                "scheduled" => a
                    .scheduled_date
                    .as_deref()
                    .cmp(&b.scheduled_date.as_deref()),
                "deadline" => a.deadline_date.as_deref().cmp(&b.deadline_date.as_deref()),
                "priority" => {
                    let a_p = a.priority.map(priority_value).unwrap_or(3);
                    let b_p = b.priority.map(priority_value).unwrap_or(3);
                    a_p.cmp(&b_p)
                }
                "file" => a.title.cmp(&b.title),
                "date" => {
                    let a_date = a
                        .scheduled_date
                        .as_deref()
                        .or(a.deadline_date.as_deref())
                        .or(a.daily_file_date.as_deref());
                    let b_date = b
                        .scheduled_date
                        .as_deref()
                        .or(b.deadline_date.as_deref())
                        .or(b.daily_file_date.as_deref());
                    a_date.cmp(&b_date)
                }
                _ => std::cmp::Ordering::Equal,
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

fn assign_canonical_ids(items: &mut [AgendaItem]) {
    items.sort_by(|a, b| {
        let a_p = a.priority.map(priority_value).unwrap_or(3);
        let b_p = b.priority.map(priority_value).unwrap_or(3);
        a_p.cmp(&b_p)
            .then(a.path.cmp(&b.path))
            .then(a.line_number.cmp(&b.line_number))
    });
    for (i, item) in items.iter_mut().enumerate() {
        item.id = i + 1;
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

fn format_agenda_rows(item: &AgendaItem) -> Vec<[String; 8]> {
    let id = if item.id > 0 {
        item.id.to_string()
    } else {
        String::new()
    };
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
            id.clone(),
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
            String::new(),
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
            id,
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

fn filter_agenda_row(row: &[String; 8], cols: &[Column]) -> Vec<String> {
    cols.iter().map(|c| row[*c as usize].clone()).collect()
}

fn print_agenda_text(items: &[AgendaItem], flat: bool, line_sep: bool, cols: &[Column]) {
    if items.is_empty() {
        println!("No planned agenda items found.");
        return;
    }

    let mut max_widths: [usize; 8] = ALL_COLUMNS.map(|c| c.name().len());
    for item in items {
        for row in format_agenda_rows(item) {
            for (i, col) in row.iter().enumerate() {
                let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                max_widths[i] = max_widths[i].max(line_w);
            }
        }
    }
    let wrap = adaptive_column_widths(cols, &max_widths);

    let today = Local::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    let mut overdue = Vec::new();
    let mut today_items = Vec::new();
    let mut upcoming = Vec::new();

    for item in items {
        if item.is_overdue {
            overdue.push(item);
        } else if item.scheduled_date.as_deref() == Some(today_str.as_str())
            || item.deadline_date.as_deref() == Some(today_str.as_str())
            || (item.is_daily_file && item.daily_file_date.as_deref() == Some(today_str.as_str()))
        {
            today_items.push(item);
        } else if item.scheduled_date.is_some()
            || item.deadline_date.is_some()
            || item.is_daily_file
        {
            upcoming.push(item);
        }
    }

    let sort_date = |a: &&AgendaItem, b: &&AgendaItem| {
        let a_date = a
            .scheduled_date
            .as_deref()
            .or(a.deadline_date.as_deref())
            .or(a.daily_file_date.as_deref());
        let b_date = b
            .scheduled_date
            .as_deref()
            .or(b.deadline_date.as_deref())
            .or(b.daily_file_date.as_deref());
        a_date.cmp(&b_date)
    };
    overdue.sort_by(sort_date);
    today_items.sort_by(sort_date);
    upcoming.sort_by(sort_date);

    let n_cols = cols.len();
    let empty_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();

    let mut builder = Builder::new();
    let headers: Vec<String> = cols.iter().map(|c| c.name().to_string()).collect();
    builder.push_record(headers);

    let mut row_idx = 1;
    let mut no_border_rows: Vec<usize> = Vec::new();
    let mut section_rows: Vec<usize> = Vec::new();

    if flat {
        for item in items {
            let rows = format_agenda_rows(item);
            for (j, row) in rows.iter().enumerate() {
                if j > 0 {
                    no_border_rows.push(row_idx);
                }
                builder.push_record(filter_agenda_row(row, cols));
                row_idx += 1;
            }
        }
    } else {
        let mut need_sep = false;
        for (section_items, label) in [
            (&overdue, "=== Overdue ==="),
            (&today_items, "=== Today ==="),
            (&upcoming, "=== Upcoming ==="),
        ] {
            if section_items.is_empty() {
                continue;
            }
            if need_sep {
                builder.push_record(empty_row.clone());
                no_border_rows.push(row_idx);
                row_idx += 1;
            }
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = label.to_string();
            builder.push_record(label_row);
            section_rows.push(row_idx);
            no_border_rows.push(row_idx);
            row_idx += 1;
            for item in section_items {
                let rows = format_agenda_rows(item);
                for (j, row) in rows.iter().enumerate() {
                    if j > 0 {
                        no_border_rows.push(row_idx);
                    }
                    builder.push_record(filter_agenda_row(row, cols));
                    row_idx += 1;
                }
            }
            need_sep = true;
        }
    }

    let mut table = builder.build();
    table.with(Style::blank());
    table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    if line_sep && row_idx > 2 {
        for i in 2..row_idx {
            if !no_border_rows.contains(&i) {
                table.with(Modify::new(Rows::one(i)).with(Border::new().top('─')));
            }
        }
    }
    if let Some(widths) = wrap {
        for (col, w) in &widths {
            let idx = cols.iter().position(|c| c == col).unwrap();
            let mut prev = 1;
            for &sec in &section_rows {
                if prev < sec {
                    table.with(
                        Modify::new(Rows::new(prev..sec).intersect(Columns::new(idx..idx + 1)))
                            .with(Width::wrap(*w).keep_words(true)),
                    );
                }
                prev = sec + 1;
            }
            if prev < row_idx {
                table.with(
                    Modify::new(Rows::new(prev..row_idx).intersect(Columns::new(idx..idx + 1)))
                        .with(Width::wrap(*w).keep_words(true)),
                );
            }
        }
    }
    for &sec_row in &section_rows {
        table.with(Modify::new((sec_row, 0)).with(Span::column(n_cols as isize)));
    }
    println!("{}", table);

    let displayed = items.len();
    println!();
    println!("Total: {displayed} planned item(s)");
}
