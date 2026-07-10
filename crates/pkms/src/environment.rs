use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct RuntimeInputs {
    variables: HashMap<OsString, OsString>,
    pub config_dir: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
    pub current_dir: Option<PathBuf>,
}

impl RuntimeInputs {
    pub fn capture() -> Self {
        Self {
            variables: std::env::vars_os().collect(),
            config_dir: dirs::config_dir(),
            home_dir: dirs::home_dir(),
            current_dir: std::env::current_dir().ok(),
        }
    }

    pub fn var_os(&self, key: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.variables.get(key.as_ref()).map(OsString::as_os_str)
    }

    pub fn var(&self, key: impl AsRef<OsStr>) -> Option<&str> {
        self.var_os(key).and_then(OsStr::to_str)
    }

    pub fn non_empty_var(&self, key: impl AsRef<OsStr>) -> Option<&str> {
        self.var(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    #[cfg(test)]
    pub fn from_values(values: &[(&str, &str)]) -> Self {
        Self {
            variables: values
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect(),
            config_dir: None,
            home_dir: None,
            current_dir: None,
        }
    }
}
