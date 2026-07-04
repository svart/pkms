use crate::config::ResolvedConfig;
use anyhow::Result;

pub use pkms_task::todoist::*;

pub fn ensure_enabled(config: &ResolvedConfig) -> Result<String> {
    config.todoist_token()
}
