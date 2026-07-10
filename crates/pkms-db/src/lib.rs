//! Note database command logic for pkms.

use pkms_org::OrgConfig;
use std::path::PathBuf;

pub mod commands;
pub mod link_check;

#[derive(Debug, Clone)]
pub struct NoteCreationConfig {
    pub org: OrgConfig,
    pub new_notes_dir: PathBuf,
}

pub(crate) fn load_graph(config: &OrgConfig) -> anyhow::Result<pkms_org::Graph> {
    pkms_org::Graph::load_from(&config.scan_config(), &config.link_resolution_context())
}
