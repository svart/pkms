use crate::environment::RuntimeInputs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod columns;
mod defaults;
mod paths;

use columns::default_columns_for;
pub use columns::{ColumnSource, ColumnView, ColumnsConfig};
pub use defaults::generate_default_config;
use defaults::{default_closed_todo_states, default_open_todo_states};
pub use paths::canonicalize_or_abs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub db_root: Option<PathBuf>,
    pub new_notes_dir: Option<PathBuf>,
    pub daily_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Option<Vec<String>>,
    pub columns: Option<ColumnsConfig>,
    pub tasks: Option<TaskConfig>,
    pub agenda: Option<AgendaConfig>,
    pub rag: Option<RagConfig>,
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
    #[cfg(any(feature = "rag", test))]
    pub rag: Option<RagConfig>,
    // Keep non-RAG builds compatible with config files that contain `[rag]`.
    #[cfg(not(any(feature = "rag", test)))]
    _rag: Option<RagConfig>,
    pub(crate) runtime: RuntimeInputs,
}

#[cfg(feature = "web")]
pub type WebCommandConfig = pkms_web::WebConfig;

#[derive(Debug, Clone)]
pub struct TaskCommandConfig {
    pub org: pkms_org::OrgConfig,
    pub task_states: pkms_task::TaskStateConfig,
    pub columns: Option<ColumnsConfig>,
}

impl TaskCommandConfig {
    pub fn load_graph(&self) -> Result<pkms_org::Graph> {
        pkms_org::Graph::load_from(&self.org.scan_config(), &self.org.link_resolution_context())
    }

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
pub struct TaskConfig {
    pub inbox: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RagConfig {
    pub rag_db: Option<PathBuf>,
    pub embedding_model: Option<String>,
    pub fastembed_model_dir: Option<PathBuf>,
}

impl Config {
    pub fn load_from(config_path: &Path) -> Result<Self> {
        tracing::debug!(path = %config_path.display(), exists = config_path.exists(), "loading config");
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))
        } else {
            Ok(Config::default())
        }
    }

    pub fn resolve(
        self,
        cli_db: Option<PathBuf>,
        runtime: RuntimeInputs,
    ) -> Result<ResolvedConfig> {
        let (db_root, db_root_source) = if let Some(db_root) = cli_db {
            (db_root, "cli")
        } else if let Some(db_root) = runtime.var_os("PKMS_DB_ROOT").map(PathBuf::from) {
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
            db_root: canonicalize_or_abs(&db_root, runtime.current_dir.as_deref()),
            new_notes_dir: self.new_notes_dir,
            daily_notes_dir: self.daily_notes_dir,
            ignore_patterns: self.ignore_patterns,
            columns: self.columns,
            tasks: self.tasks,
            agenda: self.agenda,
            #[cfg(any(feature = "rag", test))]
            rag: self.rag,
            #[cfg(not(any(feature = "rag", test)))]
            _rag: self.rag,
            runtime,
        };
        tracing::debug!(
            db_root = %resolved.db_root.display(),
            db_root_source,
            ignore_pattern_count = resolved.ignore_patterns.as_ref().map_or(0, Vec::len),
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
    #[cfg(all(test, feature = "rag"))]
    pub fn for_test_db(db_root: impl Into<PathBuf>) -> Self {
        Self {
            db_root: db_root.into(),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            rag: None,
            runtime: RuntimeInputs::default(),
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

    #[cfg(any(feature = "rag", test))]
    fn resolve_optional_configured_path(&self, path: Option<&PathBuf>) -> Option<PathBuf> {
        path.map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                self.db_root.join(path)
            }
        })
    }

    pub fn resolve_ignore_patterns(&self) -> Vec<String> {
        self.ignore_patterns.clone().unwrap_or_default()
    }

    #[cfg(feature = "rag")]
    pub fn resolved_db_root(&self) -> &Path {
        &self.db_root
    }

    pub fn org_config(&self) -> pkms_org::OrgConfig {
        pkms_org::OrgConfig {
            db_root: self.db_root.clone(),
            ignore_patterns: self.resolve_ignore_patterns(),
            home_dir: self.runtime.home_dir.clone(),
            todo_states: self.todo_states(),
        }
    }

    pub fn load_graph(&self) -> Result<pkms_org::Graph> {
        let config = self.org_config();
        pkms_org::Graph::load_from(&config.scan_config(), &config.link_resolution_context())
    }

    pub fn note_creation_config(&self) -> pkms_db::NoteCreationConfig {
        pkms_db::NoteCreationConfig {
            org: self.org_config(),
            new_notes_dir: self.resolve_new_notes_dir(),
        }
    }

    #[cfg(feature = "web")]
    pub fn web_command_config(&self) -> WebCommandConfig {
        pkms_web::WebConfig {
            org: self.org_config(),
            open_todo_states: self.open_todo_states(),
            closed_todo_states: self.closed_todo_states(),
        }
    }

    pub fn task_command_config(&self) -> TaskCommandConfig {
        TaskCommandConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
            columns: self.columns.clone(),
        }
    }

    pub fn pkms_task_config(&self) -> pkms_task::PkmsTaskConfig {
        pkms_task::PkmsTaskConfig {
            org: self.org_config(),
            task_states: self.task_state_config(),
            inbox: self.tasks.as_ref().and_then(|tasks| tasks.inbox.clone()),
            daily_notes_dir: self.resolve_daily_notes_dir(),
            daily_notes_dir_configured: self.daily_notes_dir.is_some(),
        }
    }

    #[cfg(any(feature = "rag", test))]
    pub fn resolve_rag_db(&self) -> Option<PathBuf> {
        self.resolve_optional_configured_path(self.rag.as_ref().and_then(|rag| rag.rag_db.as_ref()))
    }

    #[cfg(any(feature = "rag", test))]
    pub fn resolve_rag_fastembed_model_dir(&self) -> Option<PathBuf> {
        self.resolve_optional_configured_path(
            self.rag
                .as_ref()
                .and_then(|rag| rag.fastembed_model_dir.as_ref()),
        )
    }

    #[cfg(any(feature = "rag", test))]
    pub fn rag_embedding_model(&self) -> Option<&str> {
        self.rag
            .as_ref()
            .and_then(|rag| rag.embedding_model.as_deref())
            .map(str::trim)
            .filter(|model| !model.is_empty())
    }

    pub fn task_state_config(&self) -> pkms_task::TaskStateConfig {
        pkms_task::TaskStateConfig {
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

    pub fn runtime_inputs(&self) -> &RuntimeInputs {
        &self.runtime
    }

    pub fn resolved_info(&self) -> ConfigInfo {
        ConfigInfo {
            db_root: self.db_root.clone(),
            new_notes_dir: self.resolve_new_notes_dir(),
            daily_notes_dir: self.resolve_daily_notes_dir(),
            ignore_patterns: self.resolve_ignore_patterns(),
            has_config_file: self
                .runtime
                .config_dir
                .as_ref()
                .is_some_and(|dir| dir.join("pkms.toml").exists()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigInfo {
    pub db_root: PathBuf,
    pub new_notes_dir: PathBuf,
    pub daily_notes_dir: PathBuf,
    pub ignore_patterns: Vec<String>,
    pub has_config_file: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load_ok() {
        let config = Config::load_from(Path::new("/nonexistent-pkms-config.toml"));
        assert!(config.is_ok());
    }

    #[test]
    fn db_root_precedence_uses_cli_then_injected_environment_then_config() {
        let config = || Config {
            db_root: Some(PathBuf::from("/config/db")),
            ..Config::default()
        };
        let runtime = RuntimeInputs::from_values(&[("PKMS_DB_ROOT", "/env/db")]);

        let cli = config()
            .resolve(Some(PathBuf::from("/cli/db")), runtime.clone())
            .unwrap();
        let env = config().resolve(None, runtime).unwrap();
        let file = config().resolve(None, RuntimeInputs::default()).unwrap();

        assert_eq!(cli.db_root, PathBuf::from("/cli/db"));
        assert_eq!(env.db_root, PathBuf::from("/env/db"));
        assert_eq!(file.db_root, PathBuf::from("/config/db"));
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
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
            rag: None,
            runtime: RuntimeInputs::default(),
        };
        let patterns = config.resolve_ignore_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_canonicalize_or_abs_non_existent_absolute() {
        let p = canonicalize_or_abs(Path::new("/nonexistent_path_xyz"), None);
        assert!(p.is_absolute());
        assert_eq!(p, PathBuf::from("/nonexistent_path_xyz"));
    }

    #[test]
    fn test_canonicalize_or_abs_non_existent_relative() {
        let p = canonicalize_or_abs(
            Path::new("relative_nonexistent_path"),
            Some(Path::new("/current")),
        );
        assert!(p.is_absolute()); // joined with cwd
        assert_eq!(p, PathBuf::from("/current/relative_nonexistent_path"));
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
            rag: None,
            runtime: RuntimeInputs::default(),
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

[rag]
rag_db = ".data/rag.sqlite3"
embedding_model = "Xenova/bge-small-en-v1.5"
fastembed_model_dir = "models/bge-small"
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
        let rag = config.rag.unwrap();
        assert_eq!(rag.rag_db.as_deref(), Some(Path::new(".data/rag.sqlite3")));
        assert_eq!(
            rag.embedding_model.as_deref(),
            Some("Xenova/bge-small-en-v1.5")
        );
        assert_eq!(
            rag.fastembed_model_dir.as_deref(),
            Some(Path::new("models/bge-small"))
        );
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
    fn test_rag_config_rejects_unknown_fields() {
        assert_unknown_config_field_rejected(
            r#"
db_root = "/test/db"

[rag]
rag_db = ".data/rag.sqlite3"
rag_database = "typo.sqlite3"
"#,
            "rag_database",
        );
    }

    #[test]
    fn test_generate_default_config_is_valid_config() {
        let content = generate_default_config(Some(Path::new("/my/notes")));
        toml::from_str::<Config>(&content).unwrap();
    }

    #[test]
    fn test_resolve_rag_paths_from_config_relative_to_db_root() {
        let config = ResolvedConfig {
            db_root: PathBuf::from("/test/root"),
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: None,
            rag: Some(RagConfig {
                rag_db: Some(PathBuf::from(".data/rag.sqlite3")),
                embedding_model: Some("Xenova/bge-small-en-v1.5".to_string()),
                fastembed_model_dir: Some(PathBuf::from("models/bge-small")),
            }),
            runtime: RuntimeInputs::default(),
        };

        assert_eq!(
            config.resolve_rag_db(),
            Some(PathBuf::from("/test/root/.data/rag.sqlite3"))
        );
        assert_eq!(
            config.rag_embedding_model(),
            Some("Xenova/bge-small-en-v1.5")
        );
        assert_eq!(
            config.resolve_rag_fastembed_model_dir(),
            Some(PathBuf::from("/test/root/models/bge-small"))
        );
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
    }
}
