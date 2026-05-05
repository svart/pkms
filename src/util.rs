use std::path::Path;

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
