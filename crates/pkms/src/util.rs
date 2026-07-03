use anyhow::Result;
use std::io::{self, BufRead, IsTerminal};
use std::path::Path;

pub fn priority_value(p: char) -> u8 {
    match p {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
    }
}

pub fn path_string(path: &Path) -> String {
    path.display().to_string()
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut unit = 0;
    let mut divisor = 1u64;
    while bytes / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor *= 1024;
        unit += 1;
    }
    let whole = bytes / divisor;
    let frac = (bytes % divisor) * 10 / divisor;
    format!("{}.{} {}", whole, frac, UNITS[unit])
}

pub use pkms_org::attachments::{attachment_target_exists, resolve_attachment_path};

pub fn is_stdin_piped() -> bool {
    !io::stdin().is_terminal()
}

pub fn read_stdin_ndjson_raw() -> Result<Vec<serde_json::Value>> {
    let mut values = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)?;
        values.push(value);
    }
    Ok(values)
}

pub fn read_stdin_ndjson() -> Result<Vec<String>> {
    let values = read_stdin_ndjson_raw()?;
    let uuids: Vec<String> = values
        .iter()
        .filter_map(|v| v.get("uuid").and_then(|v| v.as_str()).map(String::from))
        .collect();
    if uuids.is_empty() {
        anyhow::bail!("No valid NDJSON lines with 'uuid' field found on stdin");
    }
    Ok(uuids)
}
