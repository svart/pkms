use anyhow::{Result, bail};
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "id", rename_all = "lowercase")]
pub enum TaskId {
    Pkms(usize),
}

impl TaskId {
    pub fn display_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => id.to_string(),
        }
    }

    pub fn source_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => id.to_string(),
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskId::Pkms(id) => write!(f, "{id}"),
        }
    }
}

impl FromStr for TaskId {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            bail!("Task ID cannot be empty");
        }

        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return parse_pkms_id(trimmed);
        }

        bail!("Unsupported task ID '{trimmed}'. Use a positive integer such as 12.")
    }
}

fn parse_pkms_id(raw: &str) -> Result<TaskId> {
    let id: usize = raw
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid PKMS task ID '{raw}'"))?;
    if id == 0 {
        bail!("PKMS task ID must be greater than zero");
    }
    Ok(TaskId::Pkms(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_id() {
        assert_eq!("12".parse::<TaskId>().unwrap(), TaskId::Pkms(12));
    }

    #[test]
    fn rejects_prefixed_ids() {
        assert!("p12".parse::<TaskId>().is_err());
        assert!("pkms:12".parse::<TaskId>().is_err());
        assert!("linear:ABC-123".parse::<TaskId>().is_err());
    }

    #[test]
    fn rejects_view_local_and_invalid_ids() {
        assert!("t4".parse::<TaskId>().is_err());
        assert!("0".parse::<TaskId>().is_err());
        assert!("linear:".parse::<TaskId>().is_err());
    }
}
