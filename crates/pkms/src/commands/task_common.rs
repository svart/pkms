use crate::output::table::{TableLayout, adaptive_table_layout, table_padding_width};
use crate::output::{ALL_COLUMNS, Column, terminal_markup};
use chrono::{NaiveDate, Timelike};
use pkms_org::org_date::parse_org_date;
pub use pkms_task::{AgendaWindow, TaskGroupField, agenda_day_section_label, apply_limit};
use std::collections::HashSet;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Object, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Padding, Span, Width};

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
    cols.iter().map(|c| row[c.index()].clone()).collect()
}

fn rendered_section_width(
    cols: &[Column],
    max_widths: &[usize; 9],
    adaptive_layout: Option<&TableLayout>,
) -> usize {
    if let Some(layout) = adaptive_layout {
        return layout.rendered_width();
    }

    let content_width = cols
        .iter()
        .map(|col| max_widths[col.index()])
        .sum::<usize>();
    content_width + table_padding_width(cols.len())
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

pub fn print_table_with_empty_message<T: RowItem>(
    sections: &[(&str, &[T])],
    cols: &[Column],
    row_separators: RowSeparatorMode,
    footer: &str,
    empty_message: &str,
) {
    let entry_rows: Vec<Vec<TaskTableEntry<'_, T>>> = sections
        .iter()
        .map(|(_, items)| items.iter().map(TaskTableEntry::Item).collect())
        .collect();
    let entry_sections: Vec<(&str, &[TaskTableEntry<'_, T>])> = sections
        .iter()
        .zip(entry_rows.iter())
        .map(|((label, _), entries)| (*label, entries.as_slice()))
        .collect();
    print_table_entries_with_empty_message(
        &entry_sections,
        cols,
        row_separators,
        footer,
        empty_message,
    );
}

pub enum TaskTableEntry<'a, T> {
    Item(&'a T),
    Divider(String),
}

pub fn print_table_entries_with_empty_message<T: RowItem>(
    sections: &[(&str, &[TaskTableEntry<'_, T>])],
    cols: &[Column],
    row_separators: RowSeparatorMode,
    footer: &str,
    empty_message: &str,
) {
    if sections
        .iter()
        .all(|(_, entries)| !has_item_entries(entries))
    {
        println!("{empty_message}");
        return;
    }

    let layout = TaskTableLayout::calculate(sections, cols);
    let records = build_task_table_records(sections, cols, &layout);
    let rendered = render_task_table(records, cols, row_separators, &layout);
    println!(
        "{}",
        terminal_markup::format_if_terminal_supported(&rendered)
    );
    println!();
    println!("{footer}");
}

fn has_item_entries<T>(entries: &[TaskTableEntry<'_, T>]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, TaskTableEntry::Item(_)))
}

struct TaskTableLayout {
    adaptive_layout: Option<TableLayout>,
    section_width: usize,
    uses_section_boxes: bool,
}

impl TaskTableLayout {
    fn calculate<T: RowItem>(
        sections: &[(&str, &[TaskTableEntry<'_, T>])],
        cols: &[Column],
    ) -> Self {
        let max_widths = max_task_table_widths(sections);
        let adaptive_layout = adaptive_table_layout(cols, &max_widths);
        let section_width = rendered_section_width(cols, &max_widths, adaptive_layout.as_ref());
        let uses_section_boxes = sections
            .iter()
            .any(|(label, entries)| !label.trim().is_empty() && has_item_entries(entries));

        Self {
            adaptive_layout,
            section_width,
            uses_section_boxes,
        }
    }
}

fn max_task_table_widths<T: RowItem>(sections: &[(&str, &[TaskTableEntry<'_, T>])]) -> [usize; 9] {
    let mut max_widths: [usize; 9] = ALL_COLUMNS.map(|c| c.name().len());
    for (_, entries) in sections {
        for entry in *entries {
            let TaskTableEntry::Item(item) = entry else {
                continue;
            };
            for row in item.format_rows() {
                for (i, col) in row.iter().enumerate() {
                    let line_w = col.lines().map(|l| l.len()).max().unwrap_or(0);
                    max_widths[i] = max_widths[i].max(line_w);
                }
            }
        }
    }

    max_widths
}

struct TaskTableRecords {
    builder: Builder,
    section_rows: Vec<usize>,
    zero_padding_rows: Vec<usize>,
    no_border_rows: Vec<usize>,
    row_end: usize,
}

fn build_task_table_records<T: RowItem>(
    sections: &[(&str, &[TaskTableEntry<'_, T>])],
    cols: &[Column],
    layout: &TaskTableLayout,
) -> TaskTableRecords {
    let mut builder = Builder::new();
    let headers: Vec<String> = cols.iter().map(|c| c.name().to_string()).collect();
    builder.push_record(headers);

    let n_cols = cols.len();
    let empty_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
    let mut section_rows: Vec<usize> = Vec::new();
    let mut zero_padding_rows: Vec<usize> = Vec::new();
    let mut no_border_rows: Vec<usize> = Vec::new();
    let mut row_idx = 1;
    let mut need_sep = false;

    for (label, entries) in sections {
        if !has_item_entries(entries) {
            continue;
        }
        let has_label = !label.trim().is_empty();
        if need_sep && !layout.uses_section_boxes {
            builder.push_record(empty_row.clone());
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        if has_label {
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = section_top_delimiter(label, layout.section_width);
            builder.push_record(label_row);
            section_rows.push(row_idx);
            zero_padding_rows.push(row_idx);
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        let mut suppress_next_border = false;
        for entry in *entries {
            match entry {
                TaskTableEntry::Item(item) => {
                    let rows = item.format_rows();
                    for (j, row) in rows.iter().enumerate() {
                        if j > 0 || suppress_next_border {
                            no_border_rows.push(row_idx);
                        }
                        builder.push_record(filter_row(row, cols));
                        row_idx += 1;
                        suppress_next_border = false;
                    }
                }
                TaskTableEntry::Divider(label) => {
                    let mut divider_row: Vec<String> =
                        std::iter::repeat_n(String::new(), n_cols).collect();
                    divider_row[0] = section_divider(label, layout.section_width);
                    builder.push_record(divider_row);
                    section_rows.push(row_idx);
                    zero_padding_rows.push(row_idx);
                    no_border_rows.push(row_idx);
                    row_idx += 1;
                    suppress_next_border = true;
                }
            }
        }
        if has_label {
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = section_bottom_delimiter(layout.section_width);
            builder.push_record(label_row);
            section_rows.push(row_idx);
            zero_padding_rows.push(row_idx);
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        need_sep = true;
    }

    TaskTableRecords {
        builder,
        section_rows,
        zero_padding_rows,
        no_border_rows,
        row_end: row_idx,
    }
}

fn section_divider(label: &str, width: usize) -> String {
    let content_width = width.saturating_sub(1).max(1);
    let fill_width = content_width
        .saturating_sub(label.chars().count() + 5)
        .max(1);
    format!(" ─── {label} {}", "─".repeat(fill_width - 1))
}

fn render_task_table(
    records: TaskTableRecords,
    cols: &[Column],
    row_separators: RowSeparatorMode,
    layout: &TaskTableLayout,
) -> String {
    let mut table = records.builder.build();
    table.with(Style::blank());
    if !records.section_rows.contains(&1) {
        table.with(Modify::new(Rows::one(1)).with(Border::new().top('─')));
    }
    if row_separators.is_enabled() && records.row_end > 2 {
        for i in 2..records.row_end {
            if !records.no_border_rows.contains(&i) {
                table.with(Modify::new(Rows::one(i)).with(Border::new().top('─')));
            }
        }
    }
    if let Some(adaptive_layout) = &layout.adaptive_layout {
        for (col, w) in adaptive_layout.column_widths() {
            let idx = cols.iter().position(|c| c == col).unwrap();
            let mut prev = 1;
            for &sec in &records.section_rows {
                if prev < sec {
                    table.with(
                        Modify::new(Rows::new(prev..sec).intersect(Columns::new(idx..idx + 1)))
                            .with(Width::wrap(*w).keep_words(true)),
                    );
                }
                prev = sec + 1;
            }
            if prev < records.row_end {
                table.with(
                    Modify::new(
                        Rows::new(prev..records.row_end).intersect(Columns::new(idx..idx + 1)),
                    )
                    .with(Width::wrap(*w).keep_words(true)),
                );
            }
        }
    }
    for &sec_row in &records.section_rows {
        table.with(Modify::new((sec_row, 0)).with(Span::column(cols.len() as isize)));
    }
    for &sec_row in &records.zero_padding_rows {
        table.with(Modify::new(Rows::one(sec_row)).with(Padding::zero()));
    }

    table.to_string()
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
