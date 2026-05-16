use crate::cli::OutputFormat;
use crate::commands::task_common::*;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::{ALL_COLUMNS, Column, OutputContext, adaptive_column_widths};
use crate::parser::{find_daily_file_date, strip_org_links};
use crate::util::priority_value;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Object, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Span, Width};

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
    let type_filters = parse_filters(opts.kind.as_deref());
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
                    .map(|s| s.display().to_string())
                    .unwrap_or_default()
            }));

            items.push(AgendaItem {
                id: 0,
                uuid: primary_uuid,
                title: note_title,
                path: path.display().to_string(),
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

    let global_ids: std::collections::HashMap<(String, usize), usize> = graph
        .all_task_entries(config)
        .into_iter()
        .map(|(id, path, line)| ((path, line), id))
        .collect();
    for item in &mut items {
        item.id = global_ids
            .get(&(item.path.clone(), item.line_number))
            .copied()
            .unwrap_or(0);
    }

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

fn print_agenda_text(items: &[AgendaItem], flat: bool, line_sep: bool, cols: &[Column]) {
    if items.is_empty() {
        println!("No planned agenda items found.");
        return;
    }

    let mut max_widths: [usize; 8] = ALL_COLUMNS.map(|c| c.name().len());
    for item in items {
        for row in format_rows(item) {
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
            let rows = format_rows(item);
            for (j, row) in rows.iter().enumerate() {
                if j > 0 {
                    no_border_rows.push(row_idx);
                }
                builder.push_record(filter_row(row, cols));
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
                let rows = format_rows(*item);
                for (j, row) in rows.iter().enumerate() {
                    if j > 0 {
                        no_border_rows.push(row_idx);
                    }
                    builder.push_record(filter_row(row, cols));
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
