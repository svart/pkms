use anyhow::Result;
use pkms_org::{OrgConfig, OrgSnapshot};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskStateConfig {
    pub valid_states: Vec<String>,
    pub open_states: Vec<String>,
    pub closed_states: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PkmsTaskConfig {
    pub org: OrgConfig,
    pub task_states: TaskStateConfig,
    pub inbox: Option<String>,
    pub daily_notes_dir: PathBuf,
    pub daily_notes_dir_configured: bool,
}

impl PkmsTaskConfig {
    pub(crate) fn load_snapshot(&self) -> Result<OrgSnapshot> {
        OrgSnapshot::load(&self.org.scan_config(), &self.org.link_resolution_context())
    }

    pub fn resolved_db_root(&self) -> &Path {
        &self.org.db_root
    }

    pub fn resolve_daily_notes_dir(&self) -> PathBuf {
        self.daily_notes_dir.clone()
    }

    pub fn task_inbox(&self) -> Result<&str> {
        self.inbox
            .as_deref()
            .map(str::trim)
            .filter(|inbox| !inbox.is_empty())
            .ok_or_else(|| anyhow::anyhow!("PKMS task inbox is not configured. Set [tasks].inbox."))
    }
}

pub(crate) fn load_graph(config: &OrgConfig) -> Result<pkms_org::Graph> {
    pkms_org::Graph::load_from(&config.scan_config(), &config.link_resolution_context())
}
