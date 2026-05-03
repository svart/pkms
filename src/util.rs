use anyhow::Result;
use serde::Serialize;
use std::path::Path;

pub fn print_count(count: usize, json: bool) {
    if json {
        println!("{}", serde_json::json!({"count": count}));
    } else {
        println!("{count}");
    }
}

pub fn print_ndjson<T: Serialize>(entries: &[T]) -> Result<()> {
    for e in entries {
        println!("{}", serde_json::to_string(e)?);
    }
    Ok(())
}

pub fn short_uuid(uuid: &str) -> &str {
    if uuid.len() > 8 { &uuid[..8] } else { uuid }
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
