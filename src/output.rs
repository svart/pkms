use crate::cli::OutputFormat;
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Id,
    Date,
    State,
    Type,
    Prio,
    Tags,
    Note,
    Heading,
}

pub const ALL_COLUMNS: &[Column; 8] = &[
    Column::Id,
    Column::Date,
    Column::State,
    Column::Type,
    Column::Prio,
    Column::Tags,
    Column::Note,
    Column::Heading,
];

impl Column {
    pub fn from_str(s: &str) -> Option<Column> {
        match s.to_lowercase().as_str() {
            "id" => Some(Column::Id),
            "date" => Some(Column::Date),
            "state" => Some(Column::State),
            "type" => Some(Column::Type),
            "prio" => Some(Column::Prio),
            "tags" => Some(Column::Tags),
            "note" => Some(Column::Note),
            "heading" => Some(Column::Heading),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Column::Id => "Id",
            Column::Date => "Date",
            Column::State => "State",
            Column::Type => "Type",
            Column::Prio => "Prio",
            Column::Tags => "Tags",
            Column::Note => "Note",
            Column::Heading => "Heading",
        }
    }
}

pub struct OutputContext {
    pub format: OutputFormat,
}

impl OutputContext {
    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json | OutputFormat::Ndjson)
    }

    pub fn print_json<T: Serialize>(&self, output: &T) -> Result<()> {
        let _ = self;
        println!("{}", serde_json::to_string_pretty(output)?);
        Ok(())
    }

    pub fn print_ndjson<T: Serialize>(&self, items: &[T]) -> Result<()> {
        let _ = self;
        for item in items {
            println!("{}", serde_json::to_string(item)?);
        }
        Ok(())
    }
}

pub fn terminal_width() -> Option<usize> {
    if let Some((w, _)) = terminal_size::terminal_size() {
        return Some(w.0 as usize);
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&w| w > 0)
}

const BASE_PADDING: usize = 5;
const COL_PADDING: usize = 2;
const MIN_COLUMN_WIDTH: usize = 15;
const TAG_WEIGHT: f64 = 0.20;
const NOTE_WEIGHT: f64 = 0.35;
const HEADING_WEIGHT: f64 = 0.45;

fn is_fixed_column(col: Column) -> bool {
    matches!(
        col,
        Column::Id | Column::Date | Column::State | Column::Type | Column::Prio
    )
}

fn is_wrap_column(col: Column) -> bool {
    matches!(col, Column::Tags | Column::Note | Column::Heading)
}

fn wrap_weight(col: Column) -> f64 {
    match col {
        Column::Tags => TAG_WEIGHT,
        Column::Note => NOTE_WEIGHT,
        Column::Heading => HEADING_WEIGHT,
        _ => 0.0,
    }
}

pub fn adaptive_column_widths(
    enabled_columns: &[Column],
    max_widths: &[usize],
) -> Option<Vec<(Column, usize)>> {
    let term_w = terminal_width()?;
    let n_columns = enabled_columns.len();
    let padding = BASE_PADDING + COL_PADDING * n_columns;

    let fixed_width: usize = enabled_columns
        .iter()
        .filter(|c| is_fixed_column(**c))
        .map(|c| max_widths[*c as usize])
        .sum();

    let available = term_w.saturating_sub(fixed_width + padding);
    let min_col = MIN_COLUMN_WIDTH;

    let wrap_cols: Vec<Column> = enabled_columns
        .iter()
        .copied()
        .filter(|c| is_wrap_column(*c))
        .collect();

    let n_wrap = wrap_cols.len();

    if n_wrap == 0 {
        return Some(
            enabled_columns
                .iter()
                .map(|c| (*c, max_widths[*c as usize]))
                .collect(),
        );
    }

    if available < n_wrap * min_col {
        return None;
    }

    let has_heading = wrap_cols.contains(&Column::Heading);

    let mut result: Vec<(Column, usize)> = Vec::new();
    let mut allocated = 0usize;

    for &col in enabled_columns {
        if is_fixed_column(col) {
            result.push((col, max_widths[col as usize]));
        }
    }

    for &col in &wrap_cols {
        if has_heading && col == Column::Heading {
            continue;
        }
        let max_cw = max_widths[col as usize];
        let weight = wrap_weight(col);
        let sibling_weight: f64 = if has_heading {
            wrap_cols
                .iter()
                .filter(|c| **c != Column::Heading)
                .map(|c| wrap_weight(*c))
                .sum()
        } else {
            wrap_cols.iter().map(|c| wrap_weight(*c)).sum()
        };
        let share = if sibling_weight > 0.0 {
            (available as f64 * weight / sibling_weight).floor() as usize
        } else {
            0
        };

        let w = if available <= max_cw {
            share.max(min_col)
        } else {
            max_cw.min(share).max(min_col)
        };
        result.push((col, w));
        allocated += w;
    }

    if has_heading {
        let mut heading_w = available.saturating_sub(allocated);
        if heading_w < min_col {
            let deficit = min_col - heading_w;
            let mut remaining_deficit = deficit;
            for (col, w) in result.iter_mut() {
                if remaining_deficit == 0 {
                    break;
                }
                if !is_wrap_column(*col) || *col == Column::Heading {
                    continue;
                }
                let can_take = w.saturating_sub(min_col);
                let take = can_take.min(remaining_deficit);
                *w -= take;
                remaining_deficit -= take;
            }
            heading_w = min_col;
        }
        result.push((Column::Heading, heading_w));
    }

    let wrap_min_check: Vec<&(Column, usize)> =
        result.iter().filter(|(c, _)| is_wrap_column(*c)).collect();
    if wrap_min_check.iter().any(|(_, w)| *w < min_col) {
        return None;
    }

    Some(result)
}
