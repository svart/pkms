use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub db_root: Option<PathBuf>,
    pub new_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
    pub columns: Option<Vec<String>>,
    pub agenda: Option<AgendaConfig>,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub db_root: PathBuf,
    pub new_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
    pub columns: Option<Vec<String>>,
    pub agenda: Option<AgendaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaConfig {
    #[serde(default = "default_open_todo_states")]
    pub open_todo_states: Vec<String>,
    #[serde(default = "default_closed_todo_states")]
    pub closed_todo_states: Vec<String>,
}

fn default_open_todo_states() -> Vec<String> {
    vec!["TODO".to_string()]
}

fn default_closed_todo_states() -> Vec<String> {
    vec!["DONE".to_string()]
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = dirs::config_dir()
            .context("Could not find XDG config directory")?
            .join("pkms.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))
        } else {
            Ok(Config {
                db_root: None,
                new_notes_dir: None,
                ignore_patterns: None,
                columns: None,
                agenda: None,
            })
        }
    }

    pub fn resolve(self, cli_db: Option<PathBuf>) -> Result<ResolvedConfig> {
        let db_root = cli_db
            .or_else(|| std::env::var("PKMS_DB_ROOT").ok().map(PathBuf::from))
            .or_else(|| self.db_root.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No database root specified. Provide --db PATH, set PKMS_DB_ROOT env var, \
                     or set db_root in ~/.config/pkms.toml"
                )
            })?;
        Ok(ResolvedConfig {
            db_root: canonicalize_or_abs(&db_root),
            new_notes_dir: self.new_notes_dir,
            ignore_patterns: self.ignore_patterns,
            columns: self.columns,
            agenda: self.agenda,
        })
    }
}

impl ResolvedConfig {
    pub fn resolve_new_notes_dir(&self) -> PathBuf {
        match &self.new_notes_dir {
            Some(dir) => {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    self.db_root.join(dir)
                }
            }
            None => self.db_root.join("roam"),
        }
    }

    pub fn resolve_ignore_patterns(&self) -> Vec<String> {
        self.ignore_patterns.clone().unwrap_or_default()
    }

    pub fn resolved_db_root(&self) -> &Path {
        &self.db_root
    }

    pub fn todo_states(&self) -> Vec<String> {
        let open = self.open_todo_states();
        let closed = self.closed_todo_states();
        let mut states = Vec::with_capacity(open.len() + closed.len());
        states.extend(open);
        states.extend(closed);
        states
    }

    pub fn open_todo_states(&self) -> Vec<String> {
        self.agenda
            .as_ref()
            .map(|a| a.open_todo_states.clone())
            .unwrap_or_else(default_open_todo_states)
    }

    pub fn closed_todo_states(&self) -> Vec<String> {
        self.agenda
            .as_ref()
            .map(|a| a.closed_todo_states.clone())
            .unwrap_or_else(default_closed_todo_states)
    }

    pub fn resolved_info(&self) -> ConfigInfo {
        ConfigInfo {
            db_root: self.db_root.clone(),
            new_notes_dir: self.resolve_new_notes_dir(),
            ignore_patterns: self.resolve_ignore_patterns(),
            has_config_file: dirs::config_dir().is_some_and(|d| d.join("pkms.toml").exists()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigInfo {
    pub db_root: PathBuf,
    pub new_notes_dir: PathBuf,
    pub ignore_patterns: Vec<String>,
    pub has_config_file: bool,
}

pub fn canonicalize_or_abs(path: &std::path::Path) -> PathBuf {
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

pub fn generate_default_config(db_root: Option<&std::path::Path>) -> String {
    let root_line = match db_root {
        Some(p) => format!(r#"db_root = "{}""#, p.display()),
        None => r#"# db_root = "/path/to/your/org/directory""#.to_string(),
    };

    format!(
        r#"# pkms configuration
{root_line}

# Directory where new notes are created (relative to db_root or absolute)
# new_notes_dir = "roam"

# Glob patterns to ignore during file discovery
# ignore_patterns = [".attach", "*.bak"]

# Default columns for todo and agenda commands (overridable by --columns flag)
# Available: Id, Date, State, Type, Prio, Tags, Note, Heading
# columns = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]

# Agenda section: configure TODO state keyword lists
# [agenda]
# open_todo_states = ["TODO"]
# closed_todo_states = ["DONE"]
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load_ok() {
        // Config::load() should always succeed (returns defaults if file missing)
        let config = Config::load();
        assert!(config.is_ok());
    }

    #[test]
    fn test_resolve_new_notes_dir_default() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            agenda: None,
        };
        assert_eq!(
            config.resolve_new_notes_dir(),
            PathBuf::from("/test/root/roam")
        );
    }

    #[test]
    fn test_resolve_new_notes_dir_absolute() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: Some(PathBuf::from("/abs/path")),
            ignore_patterns: None,
            columns: None,
            agenda: None,
        };
        assert_eq!(config.resolve_new_notes_dir(), PathBuf::from("/abs/path"));
    }

    #[test]
    fn test_resolve_new_notes_dir_relative() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: Some(PathBuf::from("subdir")),
            ignore_patterns: None,
            columns: None,
            agenda: None,
        };
        assert_eq!(
            config.resolve_new_notes_dir(),
            PathBuf::from("/test/root/subdir")
        );
    }

    #[test]
    fn test_resolve_ignore_patterns_some() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            ignore_patterns: Some(vec!["*.bak".to_string(), ".attach".to_string()]),
            columns: None,
            agenda: None,
        };
        let patterns = config.resolve_ignore_patterns();
        assert_eq!(patterns.len(), 2);
        assert!(patterns.contains(&"*.bak".to_string()));
    }

    #[test]
    fn test_resolve_ignore_patterns_none() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            agenda: None,
        };
        let patterns = config.resolve_ignore_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_canonicalize_or_abs_non_existent_absolute() {
        let p = canonicalize_or_abs(Path::new("/nonexistent_path_xyz"));
        assert!(p.is_absolute());
        assert_eq!(p, PathBuf::from("/nonexistent_path_xyz"));
    }

    #[test]
    fn test_canonicalize_or_abs_non_existent_relative() {
        let p = canonicalize_or_abs(Path::new("relative_nonexistent_path"));
        assert!(p.is_absolute()); // joined with cwd
    }

    #[test]
    fn test_generate_default_config_with_root() {
        let output = generate_default_config(Some(Path::new("/my/notes")));
        assert!(output.contains(r#"db_root = "/my/notes""#));
    }

    #[test]
    fn test_generate_default_config_without_root() {
        let output = generate_default_config(None);
        assert!(output.contains("# db_root ="));
    }

    #[test]
    fn test_resolved_info() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/actual/db"),
            new_notes_dir: Some(PathBuf::from("/notes/dir")),
            ignore_patterns: Some(vec!["*.tmp".to_string()]),
            columns: None,
            agenda: None,
        };
        let info = config.resolved_info();
        assert_eq!(info.db_root, PathBuf::from("/actual/db"));
        assert_eq!(info.new_notes_dir, PathBuf::from("/notes/dir"));
        assert_eq!(info.ignore_patterns, vec!["*.tmp".to_string()]);
    }

    #[test]
    fn test_config_load_valid_file() {
        let content = r#"
db_root = "/test/db"
new_notes_dir = "notes"
ignore_patterns = [".attach"]
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert_eq!(config.db_root, Some(PathBuf::from("/test/db")));
        assert_eq!(config.new_notes_dir, Some(PathBuf::from("notes")));
        assert_eq!(config.ignore_patterns, Some(vec![".attach".to_string()]));
    }
}
