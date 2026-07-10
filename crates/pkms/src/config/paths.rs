use std::path::{Path, PathBuf};

pub fn canonicalize_or_abs(path: &Path, current_dir: Option<&Path>) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.unwrap_or(Path::new(".")).join(path)
        }
    })
}
