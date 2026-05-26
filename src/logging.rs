use tracing_subscriber::EnvFilter;

const LOG_ENV: &str = "PKMS_LOG";
const LOG_FORMAT_ENV: &str = "PKMS_LOG_FORMAT";

pub fn init() {
    let Some(filter) = log_filter(std::env::var(LOG_ENV).ok().as_deref()) else {
        return;
    };
    let format = LogFormat::from_env(std::env::var(LOG_FORMAT_ENV).ok().as_deref());
    match format {
        LogFormat::Text => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .try_init();
        }
        LogFormat::Json => {
            let _ = tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .try_init();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFormat {
    Text,
    Json,
}

impl LogFormat {
    fn from_env(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("json") => LogFormat::Json,
            _ => LogFormat::Text,
        }
    }
}

fn log_filter(value: Option<&str>) -> Option<EnvFilter> {
    let value = value?.trim();
    if value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false") {
        return None;
    }

    let directive = match value {
        "1" => "debug",
        _ if value.eq_ignore_ascii_case("true") => "debug",
        _ => value,
    };

    EnvFilter::try_new(directive)
        .or_else(|_| EnvFilter::try_new("debug"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_filter_disabled_when_unset_or_false() {
        assert!(log_filter(None).is_none());
        assert!(log_filter(Some("")).is_none());
        assert!(log_filter(Some("0")).is_none());
        assert!(log_filter(Some("false")).is_none());
    }

    #[test]
    fn test_log_filter_accepts_boolean_enable() {
        assert!(log_filter(Some("1")).is_some());
        assert!(log_filter(Some("true")).is_some());
    }

    #[test]
    fn test_log_format_defaults_to_text() {
        assert_eq!(LogFormat::from_env(None), LogFormat::Text);
        assert_eq!(LogFormat::from_env(Some("text")), LogFormat::Text);
        assert_eq!(LogFormat::from_env(Some("json")), LogFormat::Json);
    }
}
