use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub fn load_input_target(
    input_json: Option<&PathBuf>,
    cli_target: Option<&str>,
    field: &'static str,
    error_msg: &'static str,
) -> Result<String> {
    match (cli_target, input_json) {
        (Some(t), _) => Ok(t.to_string()),
        (None, Some(path)) => {
            let content = std::fs::read_to_string(path)?;
            let params: serde_json::Value = serde_json::from_str(&content)?;
            params
                .get(field)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or_else(|| anyhow::anyhow!("No '{}' specified in JSON", field))
        }
        (None, None) => anyhow::bail!(error_msg),
    }
}

pub fn print_count(count: usize, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::json!({"count": count}));
    } else {
        println!("{}", count);
    }
    Ok(())
}

pub fn print_ndjson<T: Serialize>(entries: &[T]) -> Result<()> {
    for e in entries {
        println!("{}", serde_json::to_string(e)?);
    }
    Ok(())
}

pub fn short_uuid(uuid: &str) -> &str {
    if uuid.len() > 8 {
        &uuid[..8]
    } else {
        uuid
    }
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
