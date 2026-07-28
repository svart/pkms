use anyhow::Result;
use pkms_task::{TaskId, TaskModifierSpec};
use std::process::ExitCode;

use super::TaskRuntime;

mod pkms;

pub(in crate::commands::task) fn run_state(
    runtime: TaskRuntime<'_>,
    id: &str,
    state: &str,
    dry_run: bool,
) -> Result<()> {
    let TaskId::Pkms(canonical_id) = id.parse::<TaskId>()?;
    pkms::set_state(runtime.config, runtime.output, canonical_id, state, dry_run)
}

pub(in crate::commands::task) fn run_done(
    runtime: TaskRuntime<'_>,
    id: &str,
    dry_run: bool,
) -> Result<()> {
    let TaskId::Pkms(canonical_id) = id.parse::<TaskId>()?;
    let closed_state = runtime
        .config
        .closed_todo_states()
        .first()
        .cloned()
        .unwrap_or_else(|| "DONE".to_string());
    pkms::set_state(
        runtime.config,
        runtime.output,
        canonical_id,
        &closed_state,
        dry_run,
    )
}

pub(in crate::commands::task) fn run_add(
    runtime: TaskRuntime<'_>,
    tokens: &[String],
) -> Result<()> {
    let spec = TaskModifierSpec::parse_on(tokens, runtime.clock.today)?;
    pkms::add(runtime.config, runtime.output, &spec, runtime.clock)
}

pub(in crate::commands::task) fn run_postpone(
    runtime: TaskRuntime<'_>,
    id: &str,
    to: Option<&str>,
) -> Result<()> {
    let TaskId::Pkms(canonical_id) = id.parse::<TaskId>()?;
    pkms::postpone(
        runtime.config,
        runtime.output,
        canonical_id,
        to,
        runtime.clock,
    )
}

pub(in crate::commands::task) fn run_mod(
    runtime: TaskRuntime<'_>,
    id: &str,
    modifiers: &[String],
) -> Result<ExitCode> {
    let spec = TaskModifierSpec::parse_mod_on(modifiers, runtime.clock.today)?;
    let TaskId::Pkms(canonical_id) = id.parse::<TaskId>()?;
    pkms::mod_task(
        runtime.config,
        runtime.output,
        canonical_id,
        &spec,
        runtime.clock,
    )
}
