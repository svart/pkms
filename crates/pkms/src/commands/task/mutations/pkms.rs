use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_task::{
    TaskClock, TaskModifierSpec, add_pkms_task, mod_pkms_task, postpone_pkms_task, set_pkms_state,
};
use std::process::ExitCode;

use super::super::render;

pub(super) fn mod_task(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<ExitCode> {
    let output = mod_pkms_task(&config.pkms_task_config(), canonical_id, spec, clock)?;
    render::print_mod_output(ctx, output, clock.today)
}

pub(super) fn set_state(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    requested_state: &str,
    dry_run: bool,
) -> Result<()> {
    let output = set_pkms_state(
        &config.pkms_task_config(),
        canonical_id,
        requested_state,
        dry_run,
    )?;
    render::print_state_change(ctx, &output)
}

pub(super) fn add(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    spec: &TaskModifierSpec,
    clock: TaskClock,
) -> Result<()> {
    let item = add_pkms_task(&config.pkms_task_config(), spec, clock)?;
    render::print_add_output(ctx, item)
}

pub(super) fn postpone(
    config: &ResolvedConfig,
    ctx: &OutputContext,
    canonical_id: usize,
    to: Option<&str>,
    clock: TaskClock,
) -> Result<()> {
    let item = postpone_pkms_task(&config.pkms_task_config(), canonical_id, to, clock)?;
    render::print_mutation_output(ctx, "postpone", item)
}
