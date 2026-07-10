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
