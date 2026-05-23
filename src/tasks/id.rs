use anyhow::{Result, bail};
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "id", rename_all = "lowercase")]
pub enum TaskId {
    Pkms(usize),
    Todoist(String),
}

impl TaskId {
    pub fn display_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => format!("p{id}"),
            TaskId::Todoist(id) => format!("todoist:{id}"),
        }
    }

    pub fn source_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => id.to_string(),
            TaskId::Todoist(id) => id.clone(),
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskId::Pkms(id) => write!(f, "pkms:{id}"),
            TaskId::Todoist(id) => write!(f, "todoist:{id}"),
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

        if let Some(rest) = trimmed.strip_prefix("pkms:") {
            return parse_pkms_id(rest);
        }

        if let Some(rest) = trimmed.strip_prefix('p') {
            return parse_pkms_id(rest);
        }

        if let Some(rest) = trimmed.strip_prefix("todoist:") {
            if rest.trim().is_empty() {
                bail!("Todoist task ID cannot be empty");
            }
            return Ok(TaskId::Todoist(rest.to_string()));
        }

        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return parse_pkms_id(trimmed);
        }

        bail!("Unsupported task ID '{trimmed}'. Use 12, p12, pkms:12, or todoist:<remote-id>.")
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
    fn parses_pkms_id_forms() {
        assert_eq!("12".parse::<TaskId>().unwrap(), TaskId::Pkms(12));
        assert_eq!("p12".parse::<TaskId>().unwrap(), TaskId::Pkms(12));
        assert_eq!("pkms:12".parse::<TaskId>().unwrap(), TaskId::Pkms(12));
    }

    #[test]
    fn parses_todoist_stable_id() {
        assert_eq!(
            "todoist:123456789".parse::<TaskId>().unwrap(),
            TaskId::Todoist("123456789".to_string())
        );
    }

    #[test]
    fn rejects_view_local_and_invalid_ids() {
        assert!("t4".parse::<TaskId>().is_err());
        assert!("pkms:0".parse::<TaskId>().is_err());
        assert!("todoist:".parse::<TaskId>().is_err());
    }
}
