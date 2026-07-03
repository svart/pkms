use crate::command_context::CommandContext;
use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::Result;
use std::process::ExitCode;

mod data;
mod model;
#[path = "check/render.rs"]
mod rendering;
#[cfg(test)]
mod tests;

use data::{CheckDisplayOptions, build_check_output, collect_check_data};
pub use model::*;
pub use rendering::render_text;

pub fn run(ctx: &CommandContext<'_>, opts: &CheckOptions) -> Result<ExitCode> {
    let output = execute(ctx.config(), opts)?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &CheckCommandOutput) -> Result<ExitCode> {
    rendering::render(ctx, output)
}

pub fn execute(config: &ResolvedConfig, opts: &CheckOptions) -> Result<CheckCommandOutput> {
    ensure_remote_file_links_available(opts.checks.requests(CheckItem::RemoteFileLinks))?;

    let graph = crate::graph::Graph::load(config)?;
    let db_root = config.resolved_db_root();

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
