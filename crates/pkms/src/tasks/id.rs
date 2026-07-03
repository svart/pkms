use anyhow::{Result, bail};
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskSourceName(String);

impl TaskSourceName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskSourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TaskSourceName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for TaskSourceName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "id", rename_all = "lowercase")]
pub enum TaskId {
    Pkms(usize),
    Todoist(String),
    External { source: TaskSourceName, id: String },
}

impl TaskId {
    pub fn display_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => format!("p{id}"),
            TaskId::Todoist(id) => format!("todoist:{id}"),
            TaskId::External { source, id } => format!("{source}:{id}"),
        }
    }

    pub fn source_id(&self) -> String {
        match self {
            TaskId::Pkms(id) => id.to_string(),
            TaskId::Todoist(id) => id.clone(),
            TaskId::External { id, .. } => id.clone(),
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskId::Pkms(id) => write!(f, "pkms:{id}"),
            TaskId::Todoist(id) => write!(f, "todoist:{id}"),
            TaskId::External { source, id } => write!(f, "{source}:{id}"),
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

        if let Some((source, id)) = trimmed.split_once(':') {
            validate_external_part("source", source)?;
            validate_external_part("id", id)?;
            return Ok(TaskId::External {
                source: TaskSourceName::new(source.to_ascii_lowercase()),
                id: id.to_string(),
            });
        }

        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return parse_pkms_id(trimmed);
        }

        bail!("Unsupported task ID '{trimmed}'. Use 12, p12, pkms:12, or <source>:<remote-id>.")
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

fn validate_external_part(name: &str, value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        bail!("Task ID {name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("Task ID {name} cannot contain whitespace");
    }
    Ok(())
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
    fn parses_external_provider_id() {
        assert_eq!(
            "linear:ABC-123".parse::<TaskId>().unwrap(),
            TaskId::External {
                source: TaskSourceName::new("linear"),
                id: "ABC-123".to_string()
            }
        );
    }

    #[test]
    fn rejects_view_local_and_invalid_ids() {
        assert!("t4".parse::<TaskId>().is_err());
        assert!("pkms:0".parse::<TaskId>().is_err());
        assert!("todoist:".parse::<TaskId>().is_err());
        assert!("linear:".parse::<TaskId>().is_err());
    }
}
