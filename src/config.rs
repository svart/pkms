use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn resolved_info(&self, db_root: &PathBuf, new_notes_dir: &PathBuf) -> ConfigInfo {
        ConfigInfo {
            db_root: db_root.clone(),
            new_notes_dir: new_notes_dir.clone(),
            ignore_patterns: self.resolve_ignore_patterns(),
            has_config_file: dirs::config_dir()
                .map(|d| d.join("pkms.toml").exists())
                .unwrap_or(false),
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
