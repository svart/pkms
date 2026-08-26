use std::path::{Component, Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crate::parser::find_daily_file_date;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DailyNoteScope {
    #[default]
    CommandDefault,
    Include,
    Exclude,
}

#[derive(Debug, Clone, Default)]
pub struct ScopeFilter {
    pub db_root: PathBuf,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub path_prefix: Option<PathBuf>,
    pub daily_notes: DailyNoteScope,
    pub modified_since: Option<i64>,
}

impl ScopeFilter {
    pub fn is_active(&self) -> bool {
        !self.include_tags.is_empty()
            || !self.exclude_tags.is_empty()
            || self.path_prefix.is_some()
            || self.daily_notes != DailyNoteScope::CommandDefault
            || self.modified_since.is_some()
    }

    pub fn matches(&self, path: &Path, tags: &[String], include_dailies_by_default: bool) -> bool {
        self.matches_tags(tags)
            && self.matches_path_prefix(path)
            && self.matches_daily_scope(path, include_dailies_by_default)
            && self.matches_modified_since(path)
    }

    fn matches_tags(&self, tags: &[String]) -> bool {
        self.include_tags
            .iter()
            .all(|required| tags.iter().any(|tag| tag == required))
            && self
                .exclude_tags
                .iter()
                .all(|excluded| tags.iter().all(|tag| tag != excluded))
    }

    fn matches_path_prefix(&self, path: &Path) -> bool {
        let Some(prefix) = self.path_prefix.as_deref() else {
            return true;
        };
        if prefix.is_absolute() {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.db_root.join(path)
            };
            return absolute.starts_with(prefix);
        }
        let relative = path.strip_prefix(&self.db_root).unwrap_or(path);
        relative.starts_with(trim_current_dir(prefix))
    }

    fn matches_daily_scope(&self, path: &Path, include_dailies_by_default: bool) -> bool {
        let include = match self.daily_notes {
            DailyNoteScope::CommandDefault => include_dailies_by_default,
            DailyNoteScope::Include => true,
            DailyNoteScope::Exclude => false,
        };
        include || find_daily_file_date(path).is_none()
    }

    fn matches_modified_since(&self, path: &Path) -> bool {
        let Some(cutoff) = self.modified_since else {
            return true;
        };
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.db_root.join(path)
        };
        let Ok(modified) = std::fs::metadata(path).and_then(|metadata| metadata.modified()) else {
            return false;
        };
        let cutoff = if cutoff >= 0 {
            UNIX_EPOCH + Duration::from_secs(cutoff as u64)
        } else {
            UNIX_EPOCH
                .checked_sub(Duration::from_secs(cutoff.unsigned_abs()))
                .unwrap_or(UNIX_EPOCH)
        };
        modified >= cutoff
    }
}

fn trim_current_dir(path: &Path) -> &Path {
    let mut components = path.components();
    if components.next() == Some(Component::CurDir) {
        components.as_path()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_tags_paths_and_daily_scope() {
        let filter = ScopeFilter {
            db_root: PathBuf::from("/notes"),
            include_tags: vec!["rust".to_string()],
            exclude_tags: vec!["private".to_string()],
            path_prefix: Some(PathBuf::from("projects")),
            daily_notes: DailyNoteScope::Exclude,
            modified_since: None,
        };

        assert!(filter.matches(
            Path::new("/notes/projects/compiler.org"),
            &["rust".to_string()],
            true
        ));
        assert!(!filter.matches(
            Path::new("/notes/projects/2026-08-27.org"),
            &["rust".to_string()],
            true
        ));
        assert!(!filter.matches(
            Path::new("/notes/projects/compiler.org"),
            &["rust".to_string(), "private".to_string()],
            true
        ));
    }

    #[test]
    fn command_default_controls_daily_notes() {
        let filter = ScopeFilter::default();
        let daily = Path::new("2026-08-27.org");

        assert!(filter.matches(daily, &[], true));
        assert!(!filter.matches(daily, &[], false));
    }
}
