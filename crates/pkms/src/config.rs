use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub db_root: Option<PathBuf>,
    pub new_notes_dir: Option<PathBuf>,
    pub daily_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
    pub columns: Option<ColumnsConfig>,
    pub tasks: Option<TaskConfig>,
    pub agenda: Option<AgendaConfig>,
    pub todoist: Option<TodoistConfig>,
    pub ssh: Option<SshConfig>,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub db_root: PathBuf,
    pub new_notes_dir: Option<PathBuf>,
    pub daily_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
    pub columns: Option<ColumnsConfig>,
    pub tasks: Option<TaskConfig>,
    pub agenda: Option<AgendaConfig>,
    pub todoist: Option<TodoistConfig>,
    pub ssh: Option<SshConfig>,
}

#[derive(Debug, Clone)]
pub struct DbCommandConfig {
    pub org: pkms_org::OrgConfig,
    pub task_states: pkms_org::graph::tasks::TaskStateConfig,
    pub ssh: Option<SshConfig>,
}

#[cfg(feature = "web")]
pub type WebCommandConfig = pkms_web::WebConfig;

#[derive(Debug, Clone)]
pub struct TaskCommandConfig {
    pub org: pkms_org::OrgConfig,
    pub task_states: pkms_org::graph::tasks::TaskStateConfig,
    pub columns: Option<ColumnsConfig>,
}

impl TaskCommandConfig {
    pub fn default_columns(
        &self,
        source: ColumnSource,
        view: ColumnView,
    ) -> Result<Option<&[String]>> {
        default_columns_for(self.columns.as_ref(), source, view)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgendaConfig {
    #[serde(default = "default_open_todo_states")]
    pub open_todo_states: Vec<String>,
    #[serde(default = "default_closed_todo_states")]
    pub closed_todo_states: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TodoistConfig {
    #[serde(default)]
    pub enabled: bool,
    pub token: Option<String>,
    pub token_env: Option<String>,
    pub default_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshConfig {
    pub identity_file: Option<PathBuf>,
    pub known_hosts: Option<PathBuf>,
    pub connect_timeout_ms: Option<u64>,
    pub operation_timeout_ms: Option<u32>,
    pub max_connections: Option<usize>,
    pub agent: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    pub inbox: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ColumnsConfig {
    Global(Vec<String>),
    Matrix(ColumnMatrixConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ColumnMatrixConfig {
    pub pkms: Option<SourceColumnConfig>,
    pub todoist: Option<SourceColumnConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceColumnConfig {
    pub tasks: Option<Vec<String>>,
    pub agenda: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSource {
    Pkms,
    Todoist,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnView {
    Tasks,
    Agenda,
}

impl ColumnsConfig {
    pub fn default_for(&self, source: ColumnSource, view: ColumnView) -> Result<Option<&[String]>> {
        match self {
            ColumnsConfig::Global(columns) => Ok(Some(columns.as_slice())),
            ColumnsConfig::Matrix(matrix) => matrix.default_for(source, view),
        }
    }
}

impl ColumnMatrixConfig {
    fn default_for(&self, source: ColumnSource, view: ColumnView) -> Result<Option<&[String]>> {
        match source {
            ColumnSource::Pkms => Ok(source_default(self.pkms.as_ref(), view)),
            ColumnSource::Todoist => Ok(source_default(self.todoist.as_ref(), view)),
            ColumnSource::All => {
                let pkms = source_default(self.pkms.as_ref(), view);
                let todoist = source_default(self.todoist.as_ref(), view);
                if pkms == todoist {
                    Ok(pkms)
                } else {
                    anyhow::bail!(
                        "Ambiguous default columns for source:all. Configure matching pkms and \
                         todoist defaults for this view or pass --columns explicitly."
                    )
                }
            }
        }
    }
}

fn source_default(source: Option<&SourceColumnConfig>, view: ColumnView) -> Option<&[String]> {
    source.and_then(|source| match view {
        ColumnView::Tasks => source.tasks.as_deref(),
        ColumnView::Agenda => source.agenda.as_deref(),
    })
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

        tracing::debug!(path = %config_path.display(), exists = config_path.exists(), "loading config");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))
        } else {
            Ok(Config {
                db_root: None,
                new_notes_dir: None,
                daily_notes_dir: None,
                ignore_patterns: None,
                columns: None,
                tasks: None,
                agenda: None,
                todoist: None,
                ssh: None,
            })
        }
    }

    pub fn resolve(self, cli_db: Option<PathBuf>) -> Result<ResolvedConfig> {
        let (db_root, db_root_source) = if let Some(db_root) = cli_db {
            (db_root, "cli")
        } else if let Some(db_root) = std::env::var("PKMS_DB_ROOT").ok().map(PathBuf::from) {
            (db_root, "env")
        } else if let Some(db_root) = self.db_root.clone() {
            (db_root, "config")
        } else {
            anyhow::bail!(
                "No database root specified. Provide --db PATH, set PKMS_DB_ROOT env var, \
                 or set db_root in ~/.config/pkms.toml"
            );
        };
        let resolved = ResolvedConfig {
            db_root: canonicalize_or_abs(&db_root),
            new_notes_dir: self.new_notes_dir,
            daily_notes_dir: self.daily_notes_dir,
            ignore_patterns: self.ignore_patterns,
            columns: self.columns,
            tasks: self.tasks,
            agenda: self.agenda,
            todoist: self.todoist,
            ssh: self.ssh,
        };
        tracing::debug!(
            db_root = %resolved.db_root.display(),
            db_root_source,
            ignore_pattern_count = resolved.ignore_patterns.as_ref().map_or(0, Vec::len),
            todoist_enabled = resolved.todoist_enabled(),
            has_task_inbox = resolved
                .tasks
                .as_ref()
                .and_then(|tasks| tasks.inbox.as_ref())
                .is_some(),
            "config resolved"
        );
        Ok(resolved)
    }
}

impl ResolvedConfig {
    #[cfg(test)]
    pub fn for_test_db(db_root: impl Into<PathBuf>) -> Self {
        Self {
            db_root: db_root.into(),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        }
    }

    pub fn resolve_new_notes_dir(&self) -> PathBuf {
        self.resolve_configured_dir(self.new_notes_dir.as_ref(), "roam")
    }

    pub fn resolve_daily_notes_dir(&self) -> PathBuf {
        match &self.daily_notes_dir {
            Some(dir) => self.resolve_configured_dir(Some(dir), "roam"),
            None => self.resolve_new_notes_dir(),
        }
    }

    fn resolve_configured_dir(&self, dir: Option<&PathBuf>, default: &str) -> PathBuf {
        match dir {
            Some(dir) => {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    self.db_root.join(dir)
                }
            }
            None => self.db_root.join(default),
        }
    }

    pub fn resolve_ignore_patterns(&self) -> Vec<String> {
        self.ignore_patterns.clone().unwrap_or_default()
    }

    pub fn resolved_db_root(&self) -> &Path {
        &self.db_root
    }

    pub fn org_config(&self) -> pkms_org::OrgConfig {
        pkms_org::OrgConfig {
            db_root: self.db_root.clone(),
            new_notes_dir: Some(self.resolve_new_notes_dir()),
            daily_notes_dir: Some(self.resolve_daily_notes_dir()),
            ignore_patterns: self.resolve_ignore_patterns(),
        }
    }

    pub fn db_command_config(&self) -> DbCommandConfig {
        DbCommandConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
            ssh: self.ssh.clone(),
        }
    }

    #[cfg(feature = "web")]
    pub fn web_command_config(&self) -> WebCommandConfig {
        pkms_web::WebConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
        }
    }

    pub fn task_command_config(&self) -> TaskCommandConfig {
        TaskCommandConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
            columns: self.columns.clone(),
        }
    }

    pub fn pkms_task_config(&self) -> pkms_task::config::PkmsTaskConfig {
        pkms_task::config::PkmsTaskConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
            inbox: self.tasks.as_ref().and_then(|tasks| tasks.inbox.clone()),
            daily_notes_dir_configured: self.daily_notes_dir.is_some(),
        }
    }

    pub fn task_state_config(&self) -> pkms_org::graph::tasks::TaskStateConfig {
        pkms_org::graph::tasks::TaskStateConfig {
            valid_states: self.todo_states(),
            open_states: self.open_todo_states(),
            closed_states: self.closed_todo_states(),
        }
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

    pub fn todoist_enabled(&self) -> bool {
        self.todoist.as_ref().is_some_and(|todoist| todoist.enabled)
    }

    pub fn todoist_token_env(&self) -> &str {
        self.todoist
            .as_ref()
            .and_then(|todoist| todoist.token_env.as_deref())
            .unwrap_or("TODOIST_API_TOKEN")
    }

    pub fn todoist_default_filter(&self) -> Option<&str> {
        self.todoist
            .as_ref()
            .and_then(|todoist| todoist.default_filter.as_deref())
    }

    pub fn todoist_token(&self) -> Result<String> {
        let env_name = self.todoist_token_env();
        if let Ok(token) = std::env::var(env_name) {
            let token = token.trim();
            if !token.is_empty() {
                return Ok(token.to_string());
            }
        }

        if let Some(token) = self
            .todoist
            .as_ref()
            .and_then(|todoist| todoist.token.as_deref())
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            return Ok(token.to_string());
        }

        anyhow::bail!(
            "Todoist token is not configured. Set env var {env_name} or [todoist].token in config"
        )
    }

    pub fn todoist_api_base_url(&self) -> String {
        std::env::var("PKMS_TODOIST_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.todoist.com/api/v1".to_string())
    }

    pub fn task_inbox(&self) -> Result<&str> {
        self.tasks
            .as_ref()
            .and_then(|tasks| tasks.inbox.as_deref())
            .map(str::trim)
            .filter(|inbox| !inbox.is_empty())
            .ok_or_else(|| anyhow::anyhow!("PKMS task inbox is not configured. Set [tasks].inbox."))
    }

    pub fn default_columns(
        &self,
        source: ColumnSource,
        view: ColumnView,
    ) -> Result<Option<&[String]>> {
        default_columns_for(self.columns.as_ref(), source, view)
    }

    pub fn resolved_info(&self) -> ConfigInfo {
        ConfigInfo {
            db_root: self.db_root.clone(),
            new_notes_dir: self.resolve_new_notes_dir(),
            daily_notes_dir: self.resolve_daily_notes_dir(),
            ignore_patterns: self.resolve_ignore_patterns(),
            has_config_file: dirs::config_dir().is_some_and(|d| d.join("pkms.toml").exists()),
        }
    }
}

fn default_columns_for(
    columns: Option<&ColumnsConfig>,
    source: ColumnSource,
    view: ColumnView,
) -> Result<Option<&[String]>> {
    columns
        .map(|columns| columns.default_for(source, view))
        .transpose()
        .map(Option::flatten)
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigInfo {
    pub db_root: PathBuf,
    pub new_notes_dir: PathBuf,
    pub daily_notes_dir: PathBuf,
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

# Directory where daily notes are created (defaults to new_notes_dir)
# daily_notes_dir = "roam/daily"

# Glob patterns to ignore during file discovery
# ignore_patterns = [".attach", "*.bak"]

# Default task table columns (overridable by --columns flag)
# Available: Id, Date, State, Type, Prio, Tags, Project, Note, Heading
# Global default:
# columns = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading"]
#
# Source/view-specific defaults:
# [columns.pkms]
# tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
# agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]
#
# [columns.todoist]
# tasks = ["Id", "State", "Prio", "Tags", "Project", "Heading"]
# agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Heading"]

# Task section: configure the PKMS inbox note used by `pkms task inbox` and `pkms task add`
# [tasks]
# inbox = "Inbox"

# Agenda section: configure TODO state keyword lists
# [agenda]
# open_todo_states = ["TODO"]
# closed_todo_states = ["DONE"]

# Todoist is disabled by default. Prefer storing the token in the environment.
# [todoist]
# enabled = false
# token = "..." # optional; env var below takes precedence
# token_env = "TODOIST_API_TOKEN"
# default_filter = "today | overdue"

# SSH file-link checks are disabled unless pkms is built with --features ssh and
# `pkms check --remote-file-links` is passed. Password prompts are not used.
# [ssh]
# identity_file = "~/.ssh/id_ed25519"
# known_hosts = "~/.ssh/known_hosts"
# connect_timeout_ms = 5000
# operation_timeout_ms = 5000
# max_connections = 4
# agent = true
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
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
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
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert_eq!(config.resolve_new_notes_dir(), PathBuf::from("/abs/path"));
    }

    #[test]
    fn test_resolve_new_notes_dir_relative() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: Some(PathBuf::from("subdir")),
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert_eq!(
            config.resolve_new_notes_dir(),
            PathBuf::from("/test/root/subdir")
        );
    }

    #[test]
    fn test_resolve_daily_notes_dir_defaults_to_new_notes_dir() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: Some(PathBuf::from("notes")),
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert_eq!(
            config.resolve_daily_notes_dir(),
            PathBuf::from("/test/root/notes")
        );
    }

    #[test]
    fn test_resolve_daily_notes_dir_relative() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: Some(PathBuf::from("notes")),
            daily_notes_dir: Some(PathBuf::from("daily")),
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert_eq!(
            config.resolve_daily_notes_dir(),
            PathBuf::from("/test/root/daily")
        );
    }

    #[test]
    fn test_resolve_daily_notes_dir_absolute() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: Some(PathBuf::from("/daily/path")),
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert_eq!(
            config.resolve_daily_notes_dir(),
            PathBuf::from("/daily/path")
        );
    }

    #[test]
    fn test_resolve_ignore_patterns_some() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: Some(vec!["*.bak".to_string(), ".attach".to_string()]),
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
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
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
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
            daily_notes_dir: Some(PathBuf::from("/daily/dir")),
            ignore_patterns: Some(vec!["*.tmp".to_string()]),
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        let info = config.resolved_info();
        assert_eq!(info.db_root, PathBuf::from("/actual/db"));
        assert_eq!(info.new_notes_dir, PathBuf::from("/notes/dir"));
        assert_eq!(info.daily_notes_dir, PathBuf::from("/daily/dir"));
        assert_eq!(info.ignore_patterns, vec!["*.tmp".to_string()]);
    }

    #[test]
    fn test_config_load_valid_file() {
        let content = r#"
db_root = "/test/db"
new_notes_dir = "notes"
daily_notes_dir = "daily"
ignore_patterns = [".attach"]

[tasks]
inbox = "Inbox"

[todoist]
enabled = true
token = "config-token"
token_env = "PKMS_TEST_TODOIST_TOKEN"
default_filter = "today | overdue"

[ssh]
identity_file = "~/.ssh/id_ed25519"
known_hosts = "~/.ssh/known_hosts"
connect_timeout_ms = 7000
operation_timeout_ms = 8000
max_connections = 2
agent = false
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert_eq!(config.db_root, Some(PathBuf::from("/test/db")));
        assert_eq!(config.new_notes_dir, Some(PathBuf::from("notes")));
        assert_eq!(config.daily_notes_dir, Some(PathBuf::from("daily")));
        assert_eq!(config.ignore_patterns, Some(vec![".attach".to_string()]));
        assert_eq!(
            config
                .tasks
                .as_ref()
                .and_then(|tasks| tasks.inbox.as_deref()),
            Some("Inbox")
        );
        let todoist = config.todoist.unwrap();
        assert!(todoist.enabled);
        assert_eq!(todoist.token.as_deref(), Some("config-token"));
        assert_eq!(
            todoist.token_env.as_deref(),
            Some("PKMS_TEST_TODOIST_TOKEN")
        );
        assert_eq!(todoist.default_filter.as_deref(), Some("today | overdue"));
        let ssh = config.ssh.unwrap();
        assert_eq!(
            ssh.identity_file.as_deref(),
            Some(Path::new("~/.ssh/id_ed25519"))
        );
        assert_eq!(
            ssh.known_hosts.as_deref(),
            Some(Path::new("~/.ssh/known_hosts"))
        );
        assert_eq!(ssh.connect_timeout_ms, Some(7000));
        assert_eq!(ssh.operation_timeout_ms, Some(8000));
        assert_eq!(ssh.max_connections, Some(2));
        assert_eq!(ssh.agent, Some(false));
    }

    fn assert_unknown_config_field_rejected(content: &str, field: &str) {
        let err = toml::from_str::<Config>(content).unwrap_err().to_string();
        assert!(
            err.contains("unknown field"),
            "expected unknown field error for {field}, got: {err}"
        );
        assert!(
            err.contains(field),
            "expected error to mention {field}, got: {err}"
        );
    }

    #[test]
    fn test_agenda_config_rejects_unknown_fields() {
        assert_unknown_config_field_rejected(
            r#"
db_root = "/test/db"

[agenda]
open_todo_states = ["TODO"]
typo = true
"#,
            "typo",
        );
    }

    #[test]
    fn test_todoist_config_rejects_unknown_fields() {
        assert_unknown_config_field_rejected(
            r#"
db_root = "/test/db"

[todoist]
enabled = true
tokne = "secret"
"#,
            "tokne",
        );
    }

    #[test]
    fn test_task_config_rejects_unknown_fields() {
        assert_unknown_config_field_rejected(
            r#"
db_root = "/test/db"

[tasks]
inbox = "Inbox"
unk = "value"
"#,
            "unk",
        );
    }

    #[test]
    fn test_generate_default_config_is_valid_config() {
        let content = generate_default_config(Some(Path::new("/my/notes")));
        toml::from_str::<Config>(&content).unwrap();
    }

    #[test]
    fn test_global_columns_config_parses() {
        let config: Config = toml::from_str(
            r#"
db_root = "/test/db"
columns = ["Id", "Heading"]
"#,
        )
        .unwrap();
        assert_eq!(
            config
                .columns
                .as_ref()
                .and_then(|columns| columns
                    .default_for(ColumnSource::Pkms, ColumnView::Tasks)
                    .ok())
                .flatten(),
            Some(["Id".to_string(), "Heading".to_string()].as_slice())
        );
    }

    #[test]
    fn test_source_view_columns_config_parses() {
        let config: Config = toml::from_str(
            r#"
db_root = "/test/db"

[columns.pkms]
tasks = ["Id", "Heading"]
agenda = ["Id", "Date", "Heading"]

[columns.todoist]
tasks = ["Id", "Project", "Heading"]
agenda = ["Id", "Date", "Project", "Heading"]
"#,
        )
        .unwrap();
        let columns = config.columns.as_ref().unwrap();
        assert_eq!(
            columns
                .default_for(ColumnSource::Pkms, ColumnView::Tasks)
                .unwrap(),
            Some(["Id".to_string(), "Heading".to_string()].as_slice())
        );
        assert_eq!(
            columns
                .default_for(ColumnSource::Todoist, ColumnView::Agenda)
                .unwrap(),
            Some(
                [
                    "Id".to_string(),
                    "Date".to_string(),
                    "Project".to_string(),
                    "Heading".to_string()
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn test_source_all_columns_error_when_source_defaults_differ() {
        let config: Config = toml::from_str(
            r#"
db_root = "/test/db"

[columns.pkms]
tasks = ["Id", "Heading"]

[columns.todoist]
tasks = ["Id", "Project", "Heading"]
"#,
        )
        .unwrap();
        let columns = config.columns.as_ref().unwrap();
        assert!(
            columns
                .default_for(ColumnSource::All, ColumnView::Tasks)
                .is_err()
        );
    }

    #[test]
    fn test_todoist_config_defaults() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        };
        assert!(!config.todoist_enabled());
        assert_eq!(config.todoist_token_env(), "TODOIST_API_TOKEN");
        assert_eq!(config.todoist_default_filter(), None);
    }

    #[test]
    fn test_todoist_token_error_does_not_include_secret_value() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: Some(TodoistConfig {
                enabled: true,
                token: None,
                token_env: Some("PKMS_TEST_MISSING_TODOIST_TOKEN".to_string()),
                default_filter: None,
            }),
            ssh: None,
        };
        let error = config.todoist_token().unwrap_err().to_string();
        assert!(error.contains("PKMS_TEST_MISSING_TODOIST_TOKEN"));
    }

    #[test]
    fn test_todoist_token_can_come_from_config() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            todoist: Some(TodoistConfig {
                enabled: true,
                token: Some(" config-token ".to_string()),
                token_env: Some("PKMS_TEST_MISSING_TODOIST_TOKEN".to_string()),
                default_filter: None,
            }),
            ssh: None,
        };
        assert_eq!(config.todoist_token().unwrap(), "config-token");
    }
}
