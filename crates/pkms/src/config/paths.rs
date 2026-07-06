use std::path::{Path, PathBuf};

pub fn canonicalize_or_abs(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or(PathBuf::from("."))
                .join(path)
        }
    })
}
