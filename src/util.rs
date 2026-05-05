use anyhow::Result;
use std::io::{self, BufRead, IsTerminal};
use std::path::Path;

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn is_stdin_piped() -> bool {
    !io::stdin().is_terminal()
}

pub fn read_stdin_ndjson() -> Result<Vec<String>> {
    let mut uuids = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)?;
        if let Some(uuid) = value.get("uuid").and_then(|v| v.as_str()) {
            uuids.push(uuid.to_string());
        }
    }
    if uuids.is_empty() {
        anyhow::bail!("No valid NDJSON lines with 'uuid' field found on stdin");
    }
    Ok(uuids)
}
