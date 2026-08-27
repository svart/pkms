use crate::link_check::SshFileCheckOptions;
use anyhow::Result;
use pkms_org::OrgConfig;

mod data;
mod model;
#[path = "check/render.rs"]
mod rendering;
#[cfg(test)]
mod tests;

use data::{CheckDisplayOptions, build_check_output, collect_check_data};
pub use model::*;
pub use rendering::render_text;

#[derive(Debug, Clone)]
pub struct CheckConfig {
    pub org: OrgConfig,
    pub ssh: SshFileCheckOptions,
}

pub fn execute(config: &CheckConfig, opts: &CheckOptions) -> Result<CheckOutput> {
    #[cfg(not(feature = "ssh"))]
    ensure_remote_file_links_available(opts.checks.requests(CheckItem::RemoteFileLinks))?;

    let graph = crate::load_graph(&config.org)?;
    let db_root = config.org.db_root.as_path();

    let display_opts = CheckDisplayOptions::from_options(opts);
    let issue_data = collect_check_data(config, &graph, db_root, opts, &display_opts)?;
    Ok(build_check_output(&issue_data, &display_opts))
}

#[cfg(not(feature = "ssh"))]
fn ensure_remote_file_links_available(requested: bool) -> Result<()> {
    if requested {
        anyhow::bail!(
            "SSH file-link checks are not available in this build. Rebuild with --features ssh."
        )
    }
    Ok(())
}
