use crate::cli::OutputFormat;
use anyhow::Result;
use serde::Serialize;

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

pub fn adaptive_note_heading_widths(max_note: usize, fixed_width: usize) -> Option<(usize, usize)> {
    let term_w = terminal_width()?;
    let padding = 17;
    let available = term_w.saturating_sub(fixed_width + padding);
    let min_col = 15;

    if available < 2 * min_col {
        return None;
    }

    let thirty_five = (available as f64 * 0.35).floor() as usize;

    let (mut note_w, mut heading_w) = if available <= max_note {
        let n = thirty_five.max(min_col);
        (n, available.saturating_sub(n))
    } else if max_note <= thirty_five {
        (max_note, available - max_note)
    } else {
        (thirty_five, available - thirty_five)
    };

    if note_w < min_col {
        heading_w = heading_w.saturating_sub(min_col - note_w);
        note_w = min_col;
    }
    if heading_w < min_col {
        note_w = note_w.saturating_sub(min_col - heading_w);
        heading_w = min_col;
    }

    if note_w < min_col || heading_w < min_col {
        return None;
    }

    Some((note_w, heading_w))
}
