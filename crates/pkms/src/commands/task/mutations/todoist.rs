use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_task::clock::TaskClock;
use pkms_task::modifiers::TaskModifierSpec;
use std::process::ExitCode;

#[cfg(feature = "todoist")]
use crate::cli::OutputFormat;
#[cfg(not(feature = "todoist"))]
use anyhow::bail;
#[cfg(feature = "todoist")]
use pkms_task::config::TodoistProviderConfig;
#[cfg(feature = "todoist")]
use pkms_task::todoist_mutation::{self, TodoistDoneOutput, TodoistStateOutput};

#[cfg(feature = "todoist")]
use super::super::render;

#[cfg(feature = "todoist")]
pub(super) fn set_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    match todoist_mutation::set_todoist_state(
        &todoist_config(config)?,
        id,
        requested_state,
        dry_run,
    )? {
        TodoistStateOutput::Done(output) => print_done_output(ctx, &output),
        TodoistStateOutput::OpenDryRun { id } => {
            println!("Would reopen Todoist task {id}");
            Ok(())
        }
        TodoistStateOutput::Opened(item) => render::print_mutation_output(ctx, "state-open", *item),
    }
}

#[cfg(not(feature = "todoist"))]
pub(super) fn set_state(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _requested_state: &str,
    _dry_run: bool,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub(super) fn add(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
    _clock: TaskClock,
) -> Result<()> {
    let item = todoist_mutation::add_todoist_task(&todoist_config(config)?, spec)?;
    render::print_add_output(ctx, item)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn add(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    spec: &TaskModifierSpec,
    _clock: TaskClock,
) -> Result<()> {
    if spec.dependency.is_some() {
        bail!("dep is available only for PKMS task creation.");
    }
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub(super) fn mod_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
    let output = todoist_mutation::mod_todoist_task(&todoist_config(config)?, id, spec)?;
    render::print_mod_output(ctx, output, clock.today)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn mod_task(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _spec: &TaskModifierSpec,
    _clock: TaskClock,
) -> Result<ExitCode> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub(super) fn postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    to: &str,
    clock: TaskClock,
) -> Result<()> {
    let item = todoist_mutation::postpone_todoist_task(&todoist_config(config)?, id, to, clock)?;
    render::print_mutation_output(ctx, "postpone", item)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn postpone(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _to: &str,
    _clock: TaskClock,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
pub(super) fn close(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    id: &str,
    dry_run: bool,
) -> Result<()> {
    let output = todoist_mutation::close_todoist_task(&todoist_config(config)?, id, dry_run)?;
    print_done_output(ctx, &output)
}

#[cfg(not(feature = "todoist"))]
pub(super) fn close(
    _config: &ResolvedConfig,
    _ctx: &OutputContext,
    _id: &str,
    _dry_run: bool,
) -> Result<()> {
    bail!("Todoist support is not available in this build. Rebuild with --features todoist.")
}

#[cfg(feature = "todoist")]
fn todoist_config(config: &ResolvedConfig) -> Result<TodoistProviderConfig> {
    Ok(TodoistProviderConfig {
        org: config.org_config(),
        token: config.todoist_token()?,
        api_base_url: config.todoist_api_base_url(),
        default_filter: config.todoist_default_filter().map(str::to_string),
    })
}

#[cfg(feature = "todoist")]
fn print_done_output(ctx: &OutputContext, output: &TodoistDoneOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            if output.dry_run {
                println!("Would complete Todoist task {}", output.id);
            } else {
                println!("Completed Todoist task {}", output.id);
            }
            Ok(())
        }
        OutputFormat::Json => ctx.print_json(output),
        OutputFormat::Ndjson => ctx.print_ndjson(std::slice::from_ref(output)),
    }
}
