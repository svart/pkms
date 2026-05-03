use std::path::Path;

pub fn short_uuid(uuid: &str) -> &str {
    if uuid.len() > 8 { &uuid[..8] } else { uuid }
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
