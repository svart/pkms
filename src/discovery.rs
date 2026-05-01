use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
}

pub fn discover_files(root: &Path, ignore_patterns: &[String]) -> Result<Vec<FileEntry>> {
    let root = root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to resolve db root '{}': {}", root.display(), e))?;

    let compiled_patterns: Vec<glob::Pattern> = ignore_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let root_clone = root.clone();
    let mut entries = Vec::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| {
            if e.path() == root_clone {
                return true;
            }
            !is_ignored(e, &compiled_patterns)
        })
    {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().map_or(false, |e| e == "org") {
            entries.push(FileEntry {
                path: entry.path().to_path_buf(),
            });
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn is_ignored(entry: &walkdir::DirEntry, ignore_patterns: &[glob::Pattern]) -> bool {
    let file_name = entry.file_name().to_string_lossy();
    if file_name.starts_with('.') {
        return true;
    }
    if ignore_patterns.iter().any(|pat| pat.matches(&file_name)) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_files() {
        let dir = tempfile::tempdir().unwrap();
        let org_path = dir.path().join("note.org");
        fs::write(&org_path, "test").unwrap();
        fs::write(dir.path().join("ignored.bak"), "test").unwrap();

        let hidden = dir.path().join(".hidden");
        fs::create_dir(&hidden).unwrap();
        fs::write(hidden.join("secret.org"), "test").unwrap();

        let entries = discover_files(dir.path(), &["*.bak".to_string()]).unwrap();
        assert_eq!(entries.len(), 1, "expected 1 org file, got {}: {:?}", entries.len(), entries.iter().map(|e| e.path.display().to_string()).collect::<Vec<_>>());
        assert!(entries[0].path.ends_with("note.org"));
    }
}
