use anyhow::Result;
use pkms_org::{Graph, OrgConfig};
use std::process::ExitCode;

mod data;
mod model;
#[path = "check/render.rs"]
mod rendering;
#[cfg(test)]
mod tests;

use crate::link_check::SshFileCheckConfig;
use data::{CheckDisplayOptions, build_check_output, collect_check_data};
pub use model::*;
pub use rendering::render_text;

#[derive(Debug, Clone)]
pub struct CheckConfig {
    pub org: OrgConfig,
    pub ssh: Option<SshFileCheckConfig>,
}

pub fn execute(config: &CheckConfig, opts: &CheckOptions) -> Result<CheckCommandOutput> {
    ensure_remote_file_links_available(opts.checks.requests(CheckItem::RemoteFileLinks))?;

    let graph = Graph::load(&config.org)?;
    let db_root = config.org.db_root.as_path();

    let display_opts = CheckDisplayOptions::from_options(opts);
    let issue_data = collect_check_data(config, &graph, db_root, opts, &display_opts)?;
    let output = build_check_output(&issue_data, &display_opts);
    let exit_code = if output.healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };

    Ok(CheckCommandOutput { output, exit_code })
}

#[cfg(feature = "ssh")]
fn ensure_remote_file_links_available(_requested: bool) -> Result<()> {
    Ok(())
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
