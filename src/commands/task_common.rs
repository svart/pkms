use crate::org_date::parse_org_date;
use crate::output::{ALL_COLUMNS, Column, adaptive_column_widths};
use chrono::{Local, Timelike};
use std::collections::HashSet;
use tabled::builder::Builder;
use tabled::settings::object::{Columns, Object, Rows};
use tabled::settings::style::{Border, Style};
use tabled::settings::{Modify, Span, Width};

pub enum Filter {
    Include(String),
    Exclude(String),
}

pub fn parse_filters(s: Option<&str>) -> Vec<Filter> {
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

pub fn apply_state_filter(state: Option<&str>, filters: &[Filter]) -> bool {
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

pub fn apply_tags_filter(tags: &[String], filters: &[Filter]) -> bool {
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

pub fn apply_type_filter(has_scheduled: bool, has_deadline: bool, filters: &[Filter]) -> bool {
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

    fn effective_date(&self) -> Option<&str> {
        self.scheduled_date_str()
            .or_else(|| self.deadline_date_str())
            .or_else(|| self.daily_file_date())
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
            let date = self.daily_file_date().map(|d| d.to_string());
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

pub fn print_table<T: RowItem>(
    sections: &[(&str, &[T])],
    cols: &[Column],
    line_sep: bool,
    footer: &str,
) {
    print_table_with_empty_message(sections, cols, line_sep, footer, "No items found.");
}

pub fn print_table_with_empty_message<T: RowItem>(
    sections: &[(&str, &[T])],
    cols: &[Column],
    line_sep: bool,
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
        if need_sep {
            builder.push_record(empty_row.clone());
            no_border_rows.push(row_idx);
            row_idx += 1;
        }
        if !label.is_empty() {
            let mut label_row: Vec<String> = std::iter::repeat_n(String::new(), n_cols).collect();
            label_row[0] = label.to_string();
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
        need_sep = true;
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
