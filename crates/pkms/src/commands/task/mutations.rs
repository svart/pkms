use crate::tasks::id::TaskId;
use crate::tasks::model::TaskSourceKind;
use crate::tasks::modifiers::TaskModifierSpec;
use anyhow::Result;
#[cfg(feature = "todoist")]
pub(super) use pkms_task::mutation::{mod_date, mod_optional_text, parse_mutation_due_date};
pub(super) use pkms_task::mutation::{mod_title, unsupported_task_source, validate_mod_source};
use std::process::ExitCode;

use super::TaskRuntime;

mod pkms;
mod todoist;

pub(in crate::commands::task) fn run_state(
    runtime: TaskRuntime<'_>,
    id: &str,
    state: &str,
    dry_run: bool,
) -> Result<()> {
    match id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => {
            pkms::set_state(runtime.config, runtime.output, canonical_id, state, dry_run)
        }
        TaskId::Todoist(id) => {
            todoist::set_state(runtime.config, runtime.output, &id, state, dry_run)
        }
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(in crate::commands::task) fn run_done(
    runtime: TaskRuntime<'_>,
    id: &str,
    dry_run: bool,
) -> Result<()> {
    match id.parse::<TaskId>()? {
        TaskId::Todoist(id) => todoist::close(runtime.config, runtime.output, &id, dry_run),
        TaskId::External { source, .. } => unsupported_task_source(&source),
        TaskId::Pkms(canonical_id) => {
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
    }
}

pub(in crate::commands::task) fn run_add(
    runtime: TaskRuntime<'_>,
    tokens: &[String],
) -> Result<()> {
    let spec = TaskModifierSpec::parse_on(tokens, runtime.clock.today)?;
    match spec.source_or_default() {
        TaskSourceKind::Pkms => pkms::add(runtime.config, runtime.output, &spec, runtime.clock),
        TaskSourceKind::Todoist => {
            todoist::add(runtime.config, runtime.output, &spec, runtime.clock)
        }
    }
}

pub(in crate::commands::task) fn run_postpone(
    runtime: TaskRuntime<'_>,
    id: &str,
    to: &str,
) -> Result<()> {
    match id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => pkms::postpone(
            runtime.config,
            runtime.output,
            canonical_id,
            to,
            runtime.clock,
        ),
        TaskId::Todoist(id) => {
            todoist::postpone(runtime.config, runtime.output, &id, to, runtime.clock)
        }
        TaskId::External { source, .. } => unsupported_task_source(&source),
    }
}

pub(in crate::commands::task) fn run_mod(
    runtime: TaskRuntime<'_>,
    id: &str,
    modifiers: &[String],
) -> Result<ExitCode> {
    let spec = TaskModifierSpec::parse_mod_on(modifiers, runtime.clock.today)?;
    match id.parse::<TaskId>()? {
        TaskId::Pkms(canonical_id) => pkms::mod_task(
            runtime.config,
            runtime.output,
            canonical_id,
            &spec,
            runtime.clock,
        ),
        TaskId::Todoist(id) => {
            todoist::mod_task(runtime.config, runtime.output, &id, &spec, runtime.clock)
        }
        TaskId::External { source, .. } => {
            unsupported_task_source(&source).map(|()| ExitCode::SUCCESS)
        }
    }
}
