use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::output::OutputContext;
use crate::parser::find_daily_file_date;
use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use serde::Serialize;

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
    parsed.base_date < today
}

fn heading_is_eligible(heading: &crate::parser::Heading) -> bool {
    heading.scheduled.is_some() || heading.deadline.is_some()
}

pub struct AgendaOptions {
    pub missing_agenda: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub overdue: bool,
    pub date: Option<NaiveDate>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub today: bool,
    pub week: bool,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &AgendaOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    let today_date = Local::now().date_naive();
    let week_start = today_date
        - chrono::Duration::days((today_date.weekday().num_days_from_monday() as i64).min(6));

    let date_filter = opts.date.or({
        if opts.today {
            Some(today_date)
        } else if opts.week {
            Some(week_start)
        } else {
            None
        }
    });

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
            if !heading_is_eligible(heading) {
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

            if opts.missing_agenda && has_agenda {
                continue;
            }

            let item_scheduled_date = extract_date(heading.scheduled.as_ref());
            let item_deadline_date = extract_date(heading.deadline.as_ref());
            let item_is_overdue = is_overdue(heading.deadline.as_ref());

            if let Some(filter_date) = date_filter {
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
            let note_title = parsed.title.clone().unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });

            items.push(AgendaItem {
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

    let sort_field = opts.sort.as_deref().unwrap_or("priority");
    sort_items(&mut items, sort_field);

    let total = items.len();

    if let Some(l) = opts.limit {
        items.truncate(l);
    }

    match ctx.format {
        OutputFormat::Text => print_agenda_text(&items, total),
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

fn format_scheduled_deadline(raw: &str) -> String {
    let parsed = parse_org_date(raw);
    match parsed {
        Some(d) => d.base_date.format("%Y-%m-%d").to_string(),
        None => raw.to_string(),
    }
}

fn print_agenda_text(items: &[AgendaItem], total: usize) {
    if items.is_empty() {
        println!("No planned agenda items found.");
        return;
    }

    let overdue_items: Vec<&AgendaItem> = items.iter().filter(|i| i.is_overdue).collect();
    let upcoming_items: Vec<&AgendaItem> = items
        .iter()
        .filter(|i| {
            !i.is_overdue
                && (i.scheduled_date.is_some() || i.deadline_date.is_some() || i.is_daily_file)
        })
        .collect();

    if !overdue_items.is_empty() {
        println!("=== Overdue (deadline passed) ===");
        for item in &overdue_items {
            let prio = item
                .priority
                .map(|p| format!("[#{}] ", p))
                .unwrap_or_default();
            let todo_display = item.todo_state.as_deref().unwrap_or_default();
            println!(
                "  {}  {} {}  \u{2014} {}",
                prio, todo_display, item.heading_title, item.title
            );
            if let Some(ref d) = item.deadline {
                println!("        DEADLINE: {}", format_scheduled_deadline(d));
            }
            if let Some(ref s) = item.scheduled {
                println!("        SCHEDULED: {}", format_scheduled_deadline(s));
            }
        }
        println!();
    }

    if !upcoming_items.is_empty() {
        println!("=== Upcoming ===");
        for item in &upcoming_items {
            let date_display = item
                .scheduled_date
                .as_deref()
                .or(item.deadline_date.as_deref())
                .or(item.daily_file_date.as_deref())
                .unwrap_or("");
            let daily_mark = if item.is_daily_file { " [daily]" } else { "" };
            let prio = item
                .priority
                .map(|p| format!("[#{}] ", p))
                .unwrap_or_default();
            let todo_display = item.todo_state.as_deref().unwrap_or_default();
            println!(
                "  {}  {}{}{}  \u{2014} {}{}",
                date_display, prio, todo_display, item.heading_title, item.title, daily_mark
            );
            if let Some(ref s) = item.scheduled {
                println!("        SCHEDULED: {}", format_scheduled_deadline(s));
            }
            if let Some(ref d) = item.deadline {
                println!("        DEADLINE: {}", format_scheduled_deadline(d));
            }
        }
        println!();
    }

    println!("Total: {total} planned item(s)");
}
