use crate::org_date::parse_org_date;
use crate::output::{ALL_COLUMNS, Column, adaptive_column_widths, terminal_markup};
use crate::tasks::clock::TaskClock;
use anyhow::{Result, bail};
use chrono::{NaiveDate, Timelike};
use std::collections::HashSet;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Object, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Padding, Span, Width};

pub const TASK_SORT_FIELD_HELP: &str =
    "priority, date, scheduled, deadline, file, source, state, task, title, or project";

pub fn combine_tags(filetags: &[String], heading_tags: &[String]) -> String {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tag in filetags.iter().chain(heading_tags.iter()) {
        if seen.insert(tag.clone()) {
            result.push(tag.clone());
        }
    }
    result.join(", ")
}

pub fn extract_date(raw: Option<&String>) -> Option<String> {
    let raw = raw.as_ref()?;
    let parsed = parse_org_date(raw)?;
    Some(parsed.base_date.format("%Y-%m-%d").to_string())
}

pub fn is_overdue(raw: Option<&String>) -> bool {
    is_overdue_on(raw, TaskClock::now())
}

pub fn is_overdue_on(raw: Option<&String>, clock: TaskClock) -> bool {
    let raw = match raw {
        Some(r) => r,
        None => return false,
    };
    let parsed = match parse_org_date(raw) {
        Some(d) => d,
        None => return false,
    };
    let compare_date = parsed.base_date_end.unwrap_or(parsed.base_date);
    if compare_date < clock.today {
        return true;
    }
    if compare_date == clock.today
        && let Some(et) = parsed.time_end
    {
        return clock.now > et;
    }
    false
}

pub trait RowItem {
    fn id(&self) -> usize;
    fn display_id(&self) -> String {
        if self.id() > 0 {
            self.id().to_string()
        } else {
            String::new()
        }
    }
    fn todo_state(&self) -> Option<&str>;
    fn priority(&self) -> Option<char>;
    fn title(&self) -> &str;
    fn heading_title(&self) -> &str;
    fn project(&self) -> Option<&str> {
        None
    }
    fn filetags(&self) -> &[String];
    fn heading_tags(&self) -> &[String];
    fn scheduled(&self) -> Option<&str>;
    fn deadline(&self) -> Option<&str>;
    fn daily_file_date(&self) -> Option<&str>;
    fn scheduled_date_str(&self) -> Option<&str>;
    fn deadline_date_str(&self) -> Option<&str>;

    fn implicit_daily_file_date(&self) -> Option<&str> {
        if self.scheduled().is_none() && self.deadline().is_none() {
            self.daily_file_date()
        } else {
            None
        }
    }

    fn effective_date(&self) -> Option<&str> {
        self.scheduled_date_str()
            .or_else(|| self.deadline_date_str())
            .or_else(|| self.implicit_daily_file_date())
    }

    fn has_effective_date(&self, date: &str) -> bool {
        self.scheduled_date_str() == Some(date)
            || self.deadline_date_str() == Some(date)
            || self.implicit_daily_file_date() == Some(date)
    }

    fn sort_by_field(&self, other: &Self, field: &str) -> std::cmp::Ordering {
        match field {
            "state" => self.todo_state().cmp(&other.todo_state()),
            "file" => self.title().cmp(other.title()),
            "priority" => {
                let a_p = self
                    .priority()
                    .map(crate::util::priority_value)
                    .unwrap_or(3);
                let b_p = other
                    .priority()
                    .map(crate::util::priority_value)
                    .unwrap_or(3);
                a_p.cmp(&b_p)
            }
            "scheduled" => self.scheduled_date_str().cmp(&other.scheduled_date_str()),
            "deadline" => self.deadline_date_str().cmp(&other.deadline_date_str()),
            "date" => self.effective_date().cmp(&other.effective_date()),
            "source" => std::cmp::Ordering::Equal,
            "task" | "title" => self.heading_title().cmp(other.heading_title()),
            "project" => self.project().cmp(&other.project()),
            _ => std::cmp::Ordering::Equal,
        }
    }

    fn format_rows(&self) -> Vec<[String; 9]> {
        let id = self.display_id();
        let state = self.todo_state().unwrap_or("").to_string();
        let prio = self
            .priority()
            .map(|p| format!("[#{}]", p))
            .unwrap_or_default();
        let title = self.title().to_string();
        let heading = self.heading_title().to_string();
        let project = self.project().unwrap_or_default().to_string();
        let tags = combine_tags(self.filetags(), self.heading_tags());
        let has_both = self.scheduled().is_some() && self.deadline().is_some();
        let mut rows = Vec::new();

        if let Some(s) = self.scheduled() {
            rows.push([
                id.clone(),
                format_display_datetime(s),
                state.clone(),
                "SCHED".to_string(),
                prio.clone(),
                tags.clone(),
                project.clone(),
                title.clone(),
                heading.clone(),
            ]);
        }

        if let Some(d) = self.deadline() {
            rows.push([
                if has_both { String::new() } else { id.clone() },
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
                    project.clone()
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

        if rows.is_empty() {
            let date = self.daily_file_date().map(format_display_date);
            rows.push([
                id,
                date.unwrap_or_default(),
                state,
                String::new(),
                prio,
                tags,
                project,
                title,
                heading,
            ]);
        }

        rows
    }
}

pub fn filter_row(row: &[String; 9], cols: &[Column]) -> Vec<String> {
    cols.iter().map(|c| row[*c as usize].clone()).collect()
}

pub fn sort_items<T: RowItem>(items: &mut [T], sort_fields: &[&str]) {
    items.sort_by(|a, b| {
        for field in sort_fields {
            let ord = a.sort_by_field(b, field);
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

pub fn apply_limit<T>(items: &mut Vec<T>, limit: Option<usize>) -> usize {
    let total = items.len();
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    total
}

pub fn agenda_window_cutoff(today: NaiveDate, days: i64) -> Option<NaiveDate> {
    if days <= 0 {
        return None;
    }
    chrono::Duration::try_days(days - 1).and_then(|duration| today.checked_add_signed(duration))
}

pub fn date_in_agenda_window(date: NaiveDate, today: NaiveDate, days: i64) -> bool {
    agenda_window_cutoff(today, days).is_some_and(|cutoff| date >= today && date <= cutoff)
}

pub fn agenda_day_section_label(today: NaiveDate, date: NaiveDate) -> String {
    match (date - today).num_days() {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ => date.format("%Y-%m-%d %a").to_string(),
    }
}

fn rendered_section_width(
    cols: &[Column],
    max_widths: &[usize; 9],
    wrap: Option<&[(Column, usize)]>,
) -> usize {
    let content_width = cols
        .iter()
        .map(|col| {
            wrap.and_then(|widths| {
                widths
                    .iter()
                    .find_map(|(wrapped_col, width)| (*wrapped_col == *col).then_some(*width))
            })
            .unwrap_or(max_widths[*col as usize])
        })
        .sum::<usize>();
    content_width + cols.len().saturating_sub(1) + 2 * cols.len()
}

fn section_top_delimiter(label: &str, width: usize) -> String {
    let title = label.trim();
    let fill_width = width.saturating_sub(title.chars().count() + 7).max(1);
    format!("╭─── {title} {}╮", "─".repeat(fill_width))
}

fn section_bottom_delimiter(width: usize) -> String {
    let fill_width = width.saturating_sub(2).max(1);
    format!("╰{}╯", "─".repeat(fill_width))
}

pub fn parse_task_sort_fields(sort: &str) -> Result<Vec<&str>> {
    let fields: Vec<&str> = sort
        .split(',')
        .map(|field| field.trim())
        .filter(|field| !field.is_empty())
        .collect();
    if fields.is_empty() {
        bail!("Task sort must include at least one field");
    }
    for field in &fields {
        match *field {
            "priority" | "date" | "scheduled" | "deadline" | "file" | "source" | "state"
            | "task" | "title" | "project" => {}
            other => bail!("Unknown task sort field '{other}'. Use {TASK_SORT_FIELD_HELP}."),
        }
    }
    Ok(fields)
}

pub fn validate_task_group_field(group_field: &str) -> Result<()> {
    match group_field {
        "state" | "file" | "priority" => Ok(()),
        other => bail!("Unknown task group field '{other}'. Use state, file, or priority."),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowSeparatorMode {
    Off,
    On,
}

impl From<bool> for RowSeparatorMode {
    fn from(enabled: bool) -> Self {
        if enabled {
            RowSeparatorMode::On
        } else {
            RowSeparatorMode::Off
        }
    }
}

impl RowSeparatorMode {
    fn is_enabled(self) -> bool {
        matches!(self, RowSeparatorMode::On)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgendaWindow {
    Sections,
    Days(i64),
}

impl AgendaWindow {
    pub fn from_days(days: Option<i64>) -> Self {
        days.map(|days| AgendaWindow::Days(days.max(0)))
            .unwrap_or(AgendaWindow::Sections)
    }

    pub fn days(self) -> Option<i64> {
        match self {
            AgendaWindow::Sections => None,
            AgendaWindow::Days(days) => Some(days),
        }
    }
}

pub fn print_table<T: RowItem>(
    sections: &[(&str, &[T])],
    cols: &[Column],
    row_separators: RowSeparatorMode,
    footer: &str,
) {
    print_table_with_empty_message(sections, cols, row_separators, footer, "No items found.");
}

pub fn print_table_with_empty_message<T: RowItem>(
    sections: &[(&str, &[T])],
    cols: &[Column],
    row_separators: RowSeparatorMode,
    footer: &str,
    empty_message: &str,
) {
    if sections.iter().all(|(_, items)| items.is_empty()) {
        println!("{empty_message}");
        return;
    }

    let mut max_widths: [usize; 9] = ALL_COLUMNS.map(|c| c.name().len());
    for (_, items) in sections {
        for item in *items {
            for row in item.format_rows() {
                for (i, col) in row.iter().enumerate() {
                    let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                    max_widths[i] = max_widths[i].max(line_w);
                }
            }
        }
    }
    let wrap = adaptive_column_widths(cols, &max_widths);
    let section_width = rendered_section_width(cols, &max_widths, wrap.as_deref());
    let uses_section_boxes = sections
        .iter()
        .any(|(label, items)| !label.trim().is_empty() && !items.is_empty());

    let mut builder = Builder::new();
    let headers: Vec<String> = cols.iter().map(|c| c.name().to_string()).collect();
    builder.push_record(headers);

    let n_cols = cols.len();
    let empty_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
    let mut section_rows: Vec<usize> = Vec::new();
    let mut no_border_rows: Vec<usize> = Vec::new();
    let mut row_idx = 1;
    let mut need_sep = false;

    for (label, items) in sections {
        if items.is_empty() {
            continue;
        }
        if need_sep && !uses_section_boxes {
            builder.push_record(empty_row.clone());
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        if !label.trim().is_empty() {
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = section_top_delimiter(label, section_width);
            builder.push_record(label_row);
            section_rows.push(row_idx);
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        for item in *items {
            let rows = item.format_rows();
            for (j, row) in rows.iter().enumerate() {
                if j > 0 {
                    no_border_rows.push(row_idx);
                }
                builder.push_record(filter_row(row, cols));
                row_idx += 1;
            }
        }
        if !label.trim().is_empty() {
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = section_bottom_delimiter(section_width);
            builder.push_record(label_row);
            section_rows.push(row_idx);
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        need_sep = true;
    }

    let mut table = builder.build();
    table.with(Style::blank());
    if !section_rows.contains(&1) {
        table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    }
    if row_separators.is_enabled() && row_idx > 2 {
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
        table.with(Modify::new(Rows::one(sec_row)).with(Padding::zero()));
    }
    let rendered = table.to_string();
    println!(
        "{}",
        terminal_markup::format_if_terminal_supported(&rendered)
    );
    println!();
    println!("{footer}");
}

pub fn format_display_datetime(raw: &str) -> String {
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

fn format_display_date(raw: &str) -> String {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(|date| date.format("%Y-%m-%d %a").to_string())
        .unwrap_or_else(|_| raw.to_string())
}
