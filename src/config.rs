use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub db_root: Option<PathBuf>,
    pub new_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
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
            })
        }
    }

    pub fn resolve_db_root(&self, cli_override: Option<&std::path::Path>) -> Result<PathBuf> {
        if let Some(path) = cli_override {
            return Ok(canonicalize_or_abs(path));
        }
        if let Some(ref path) = self.db_root {
            return Ok(canonicalize_or_abs(path));
        }
        anyhow::bail!(
            "No database root specified. Provide --db PATH or set db_root in ~/.config/pkms.toml"
        );
    }

    pub fn resolve_new_notes_dir(&self, db_root: &std::path::Path) -> PathBuf {
        match &self.new_notes_dir {
            Some(dir) => {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    db_root.join(dir)
                }
            }
            None => db_root.join("roam"),
        }
    }

    pub fn resolve_ignore_patterns(&self) -> Vec<String> {
        self.ignore_patterns.clone().unwrap_or_default()
    }

    pub fn resolved_info(&self, db_root: &Path, new_notes_dir: &Path) -> ConfigInfo {
        ConfigInfo {
            db_root: db_root.to_path_buf(),
            new_notes_dir: new_notes_dir.to_path_buf(),
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

fn canonicalize_or_abs(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
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
    fn test_resolve_db_root_with_cli_override() {
        let config = Config {
            db_root: Some(PathBuf::from("/nonexistent/config/path")),
            new_notes_dir: None,
            ignore_patterns: None,
        };
        let result = config.resolve_db_root(Some(Path::new("/cli/path")));
        // CLI override wins, even though /cli/path doesn't exist -> canonicalize_or_abs
        // returns it as-is since it's absolute
        assert!(result.is_ok());
        let p = result.unwrap();
        assert_eq!(p, PathBuf::from("/cli/path"));
    }

    #[test]
    fn test_resolve_db_root_with_config_value() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            db_root: Some(dir.path().to_path_buf()),
            new_notes_dir: None,
            ignore_patterns: None,
        };
        let result = config.resolve_db_root(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir.path().canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_db_root_none() {
        let config = Config {
            db_root: None,
            new_notes_dir: None,
            ignore_patterns: None,
        };
        let result = config.resolve_db_root(None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No database root specified")
        );
    }

    #[test]
    fn test_resolve_new_notes_dir_default() {
        let config = Config {
            db_root: None,
            new_notes_dir: None,
            ignore_patterns: None,
        };
        let db_root = Path::new("/test/root");
        assert_eq!(
            config.resolve_new_notes_dir(db_root),
            PathBuf::from("/test/root/roam")
        );
    }

    #[test]
    fn test_resolve_new_notes_dir_absolute() {
        let config = Config {
            db_root: None,
            new_notes_dir: Some(PathBuf::from("/abs/path")),
            ignore_patterns: None,
        };
        let db_root = Path::new("/test/root");
        assert_eq!(
            config.resolve_new_notes_dir(db_root),
            PathBuf::from("/abs/path")
        );
    }

    #[test]
    fn test_resolve_new_notes_dir_relative() {
        let config = Config {
            db_root: None,
            new_notes_dir: Some(PathBuf::from("subdir")),
            ignore_patterns: None,
        };
        let db_root = Path::new("/test/root");
        assert_eq!(
            config.resolve_new_notes_dir(db_root),
            PathBuf::from("/test/root/subdir")
        );
    }

    #[test]
    fn test_resolve_ignore_patterns_some() {
        let config = Config {
            db_root: None,
            new_notes_dir: None,
            ignore_patterns: Some(vec!["*.bak".to_string(), ".attach".to_string()]),
        };
        let patterns = config.resolve_ignore_patterns();
        assert_eq!(patterns.len(), 2);
        assert!(patterns.contains(&"*.bak".to_string()));
    }

    #[test]
    fn test_resolve_ignore_patterns_none() {
        let config = Config {
            db_root: None,
            new_notes_dir: None,
            ignore_patterns: None,
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
        let config = Config {
            db_root: Some(PathBuf::from("/db")),
            new_notes_dir: None,
            ignore_patterns: Some(vec!["*.tmp".to_string()]),
        };
        let info = config.resolved_info(Path::new("/actual/db"), Path::new("/notes/dir"));
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
