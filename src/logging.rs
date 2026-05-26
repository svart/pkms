use tracing_subscriber::EnvFilter;

const LOG_ENV: &str = "PKMS_LOG";
const LOG_FORMAT_ENV: &str = "PKMS_LOG_FORMAT";
const HTTP_LOG_ENV: &str = "PKMS_LOG_HTTP";
const TODOIST_HTTP_DIRECTIVE: &str = "pkms::tasks::todoist::http=debug";

pub fn init() {
    let Some(filter) = log_filter(
        std::env::var(LOG_ENV).ok().as_deref(),
        std::env::var(HTTP_LOG_ENV).ok().as_deref(),
    ) else {
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

fn log_filter(log_value: Option<&str>, http_value: Option<&str>) -> Option<EnvFilter> {
    let directive = log_directive(log_value, http_value)?;
    EnvFilter::try_new(directive)
        .or_else(|_| EnvFilter::try_new("debug"))
        .ok()
}

fn log_directive(log_value: Option<&str>, http_value: Option<&str>) -> Option<String> {
    let mut directives = Vec::new();
    if let Some(directive) = base_log_directive(log_value) {
        directives.push(directive);
    }
    if enabled(http_value) {
        directives.push(TODOIST_HTTP_DIRECTIVE.to_string());
    }

    if directives.is_empty() {
        None
    } else {
        Some(directives.join(","))
    }
}

fn base_log_directive(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false") {
        return None;
    }

    Some(
        match value {
            "1" => "debug",
            _ if value.eq_ignore_ascii_case("true") => "debug",
            _ => value,
        }
        .to_string(),
    )
}

fn enabled(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim) else {
        return false;
    };
    !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_filter_disabled_when_unset_or_false() {
        assert!(log_filter(None, None).is_none());
        assert!(log_filter(Some(""), None).is_none());
        assert!(log_filter(Some("0"), None).is_none());
        assert!(log_filter(Some("false"), None).is_none());
    }

    #[test]
    fn test_log_filter_accepts_boolean_enable() {
        assert!(log_filter(Some("1"), None).is_some());
        assert!(log_filter(Some("true"), None).is_some());
    }

    #[test]
    fn test_http_log_env_enables_todoist_http_target() {
        assert_eq!(
            log_directive(None, Some("1")).as_deref(),
            Some(TODOIST_HTTP_DIRECTIVE)
        );
        assert_eq!(
            log_directive(Some("pkms::config=debug"), Some("true")).as_deref(),
            Some("pkms::config=debug,pkms::tasks::todoist::http=debug")
        );
    }

    #[test]
    fn test_log_format_defaults_to_text() {
        assert_eq!(LogFormat::from_env(None), LogFormat::Text);
        assert_eq!(LogFormat::from_env(Some("text")), LogFormat::Text);
        assert_eq!(LogFormat::from_env(Some("json")), LogFormat::Json);
    }
}
