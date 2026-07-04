use crate::model::TaskDateValue;
use crate::modifiers::{TaskDateArg, TaskModifierSpec, is_clear_value, parse_task_date_arg_on};
use anyhow::{Result, bail};
use chrono::NaiveDate;

pub fn validate_mod_source(spec: &TaskModifierSpec, expected: &str) -> Result<()> {
    if let Some(source) = spec.source
        && !source.as_str().eq_ignore_ascii_case(expected)
    {
        bail!("Task source cannot be changed by task mod.");
    }
    Ok(())
}

pub fn mod_title(spec: &TaskModifierSpec) -> Result<Option<String>> {
    Ok(spec
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string))
}

pub fn mod_optional_text(value: Option<&str>) -> Option<Option<String>> {
    value.map(|value| {
        let value = value.trim();
        (!is_clear_value(value)).then(|| value.to_string())
    })
}

pub fn mod_date(value: Option<&TaskDateArg>) -> Option<Option<TaskDateValue>> {
    value.map(|date| date.as_value().cloned())
}

pub fn parse_mutation_due_date(value: &str, today: NaiveDate) -> Result<String> {
    Ok(parse_task_date_arg_on("due", value, today)?.to_string())
}

pub fn unsupported_task_source(source: impl AsRef<str>) -> Result<()> {
    bail!(
        "Task source '{}' is not configured in this build.",
        source.as_ref()
    )
}
