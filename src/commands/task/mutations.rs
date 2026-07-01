use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDateValue, TaskSourceKind};
use crate::tasks::modifiers::{TaskModifierSpec, is_clear_value, parse_task_date_arg_on};
use anyhow::{Result, bail};
use chrono::NaiveDate;
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
    let spec = TaskModifierSpec::parse(tokens)?;
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
    let spec = TaskModifierSpec::parse_mod(modifiers)?;
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

pub(super) fn validate_mod_source(spec: &TaskModifierSpec, expected: &str) -> Result<()> {
    if let Some(source) = spec.source
        && !source.as_str().eq_ignore_ascii_case(expected)
    {
        bail!("Task source cannot be changed by task mod.");
    }
    Ok(())
}

pub(super) fn mod_title(spec: &TaskModifierSpec) -> Result<Option<String>> {
    Ok(spec
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string))
}

pub(super) fn mod_optional_text(value: Option<&str>) -> Option<Option<String>> {
    value.map(|value| {
        let value = value.trim();
        (!is_clear_value(value)).then(|| value.to_string())
    })
}

pub(super) fn mod_date(
    name: &str,
    value: Option<&str>,
    today: NaiveDate,
) -> Result<Option<Option<TaskDateValue>>> {
    let Some(value) = value.map(str::trim) else {
        return Ok(None);
    };
    if is_clear_value(value) {
        return Ok(Some(None));
    }
    Ok(Some(Some(parse_task_date_arg_on(name, value, today)?)))
}

pub(super) fn parse_mutation_due_date(value: &str, today: NaiveDate) -> Result<String> {
    Ok(parse_task_date_arg_on("due", value, today)?.to_string())
}

pub(in crate::commands::task) fn unsupported_task_source(source: impl AsRef<str>) -> Result<()> {
    bail!(
        "Task source '{}' is not configured in this build.",
        source.as_ref()
    )
}
