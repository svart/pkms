use anyhow::Result;
use std::io::{self, BufRead, IsTerminal};
use std::path::{Path, PathBuf};

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn resolve_attachment_path(db_root: &Path, uuid: &str, target: &str) -> PathBuf {
    db_root.join(".attach").join(uuid).join(target)
}

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
