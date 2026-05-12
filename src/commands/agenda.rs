use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::{OutputContext, adaptive_note_heading_widths};
use crate::parser::{find_daily_file_date, strip_org_links};
use anyhow::Result;
use chrono::{Local, NaiveDate, Timelike};
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Width};

#[derive(Debug, Clone, Serialize)]
pub struct AgendaItem {
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
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub overdue: bool,
    pub upcoming: bool,
    pub date: Option<NaiveDate>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub today: bool,
    pub week: bool,
    pub line_sep: bool,
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
                uuid: primary_uuid,
                title: note_title,
                path: path.to_string_lossy().to_string(),
                filetags: parsed.filetags.clone(),
                has_agenda_tag: has_agenda,
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

    let sort_field = opts.sort.as_deref().unwrap_or("priority");
    sort_items(&mut items, sort_field);

    let total = items.len();

    if let Some(l) = opts.limit {
        items.truncate(l);
    }

    match ctx.format {
        OutputFormat::Text => print_agenda_text(
            &items,
            opts.today || opts.upcoming || opts.overdue,
            opts.line_sep,
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

fn sort_items(items: &mut [AgendaItem], sort_field: &str) {
    match sort_field {
        "scheduled" => {
            items.sort_by(|a, b| {
                a.scheduled_date
                    .as_deref()
                    .cmp(&b.scheduled_date.as_deref())
                    .then_with(|| a.deadline_date.as_deref().cmp(&b.deadline_date.as_deref()))
            });
        }
        "deadline" => {
            items.sort_by(|a, b| {
                a.deadline_date
                    .as_deref()
                    .cmp(&b.deadline_date.as_deref())
                    .then_with(|| {
                        a.scheduled_date
                            .as_deref()
                            .cmp(&b.scheduled_date.as_deref())
                    })
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

fn format_agenda_rows(item: &AgendaItem) -> Vec<[String; 6]> {
    let state = item.todo_state.as_deref().unwrap_or("").to_string();
    let prio = item
        .priority
        .map(|p| format!("[#{}]", p))
        .unwrap_or_default();
    let title = item.title.clone();
    let heading = item.heading_title.clone();
    let has_both = item.scheduled.is_some() && item.deadline.is_some();
    let mut rows = Vec::new();

    if let Some(ref s) = item.scheduled {
        rows.push([
            format_display_datetime(s),
            state.clone(),
            "SCHED".to_string(),
            prio.clone(),
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
        rows.push([dfd.clone(), state, String::new(), prio, title, heading]);
    }

    rows
}

fn print_agenda_text(items: &[AgendaItem], flat: bool, line_sep: bool) {
    if items.is_empty() {
        println!("No planned agenda items found.");
        return;
    }

    let headers = ["Date", "State", "Type", "Prio", "Note", "Heading"];
    let mut max_widths: [usize; 6] = headers.map(|h| h.len());
    for item in items {
        for row in format_agenda_rows(item) {
            for (i, col) in row.iter().enumerate() {
                let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                max_widths[i] = max_widths[i].max(line_w);
            }
        }
    }

    let fixed_sum = max_widths[0] + max_widths[1] + max_widths[2] + max_widths[3];
    let wrap = adaptive_note_heading_widths(max_widths[4], fixed_sum);

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

    let mut builder = Builder::new();
    builder.push_record(["Date", "State", "Type", "Prio", "Note", "Heading"]);

    let mut row_idx = 1;
    let mut no_border_rows: Vec<usize> = Vec::new();

    if flat {
        for item in items {
            let rows = format_agenda_rows(item);
            for (j, row) in rows.iter().enumerate() {
                if j > 0 {
                    no_border_rows.push(row_idx);
                }
                builder.push_record(row.clone());
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
                builder.push_record(["", "", "", "", "", ""]);
                no_border_rows.push(row_idx);
                row_idx += 1;
            }
            builder.push_record([label, "", "", "", "", ""]);
            no_border_rows.push(row_idx);
            row_idx += 1;
            for item in section_items {
                let rows = format_agenda_rows(item);
                for (j, row) in rows.iter().enumerate() {
                    if j > 0 {
                        no_border_rows.push(row_idx);
                    }
                    builder.push_record(row.clone());
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
    if let Some((note_w, heading_w)) = wrap {
        table.with(Modify::new(Columns::new(4..5)).with(Width::wrap(note_w).keep_words(true)));
        table.with(Modify::new(Columns::new(5..6)).with(Width::wrap(heading_w).keep_words(true)));
    }
    println!("{}", table);

    let displayed = items.len();
    println!();
    println!("Total: {displayed} planned item(s)");
}
