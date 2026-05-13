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

pub fn adaptive_column_widths(
    max_tags: usize,
    max_note: usize,
    fixed_width: usize,
) -> Option<(usize, usize, usize)> {
    let term_w = terminal_width()?;
    let padding = 19;
    let available = term_w.saturating_sub(fixed_width + padding);
    let min_col = 15;

    if available < 3 * min_col {
        return None;
    }

    let tags_share = (available as f64 * 0.20).floor() as usize;
    let note_share = (available as f64 * 0.35).floor() as usize;

    let (mut tags_w, mut note_w) = if available <= max_tags.max(max_note) {
        (tags_share.max(min_col), note_share.max(min_col))
    } else {
        let t = max_tags.min(tags_share).max(min_col);
        let n = max_note.min(note_share).max(min_col);
        (t, n)
    };

    let mut heading_w = available.saturating_sub(tags_w + note_w);

    if heading_w < min_col {
        let deficit = min_col - heading_w;
        let from_tags = (tags_w - min_col).min(deficit / 2);
        let from_note = (note_w - min_col).min(deficit - from_tags);
        tags_w -= from_tags;
        note_w -= from_note;
        heading_w = min_col;
    }

    if tags_w < min_col || note_w < min_col || heading_w < min_col {
        return None;
    }

    Some((tags_w, note_w, heading_w))
}
