use chrono::{NaiveDate, NaiveTime};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize)]
pub struct OrgDate {
    pub base_date: NaiveDate,
    pub has_time: bool,
    pub time: Option<NaiveTime>,
    pub repeater: Option<String>,
    pub warning: Option<String>,
    pub raw: String,
}

static ORG_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^<(\d{4}-\d{2}-\d{2})(?:\s+[A-Z][a-z]+)?(?:\s+(\d{2}:\d{2}))?(?:\s+((?:\.\+|\+\+|\+)\d+[wdmy]))?(?:\s+(-\d+[wdmy]))?>$",
    )
    .unwrap()
});

pub fn parse_org_date(raw: &str) -> Option<OrgDate> {
    let trimmed = raw.trim();
    let cap = ORG_DATE_RE.captures(trimmed)?;
    let base_date = NaiveDate::parse_from_str(cap.get(1)?.as_str(), "%Y-%m-%d").ok()?;

    let time_str = cap.get(2).map(|m| m.as_str());
    let has_time = time_str.is_some();
    let time = time_str.and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok());
    let repeater = cap.get(3).map(|m| m.as_str().to_string());
    let warning = cap.get(4).map(|m| m.as_str().to_string());

    Some(OrgDate {
        base_date,
        has_time,
        time,
        repeater,
        warning,
        raw: raw.to_string(),
    })
}

#[allow(dead_code)]
pub fn extract_timestamp(raw: &str) -> Option<String> {
    org_timestamp_re().find(raw).map(|m| m.as_str().to_string())
}

fn org_timestamp_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<(\d{4}-\d{2}-\d{2}(?:\s+[A-Z][a-z]+)?(?:\s+\d{2}:\d{2})?(?:\s+[+-]+\.?\d+[wdmy])?(?:\s+-?\d+[wdmy])?)>").unwrap()
    });
    &RE
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_simple_date() {
        let d = parse_org_date("<2026-05-10 Sun>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(!d.has_time);
        assert!(d.time.is_none());
        assert!(d.repeater.is_none());
        assert!(d.warning.is_none());
    }

    #[test]
    fn test_date_with_time() {
        let d = parse_org_date("<2026-05-10 Sun 14:00>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(14, 0, 0).unwrap());
    }

    #[test]
    fn test_with_repeater() {
        let d = parse_org_date("<2026-05-10 Sun +1w>").unwrap();
        assert_eq!(d.repeater, Some("+1w".to_string()));

        let d = parse_org_date("<2026-05-10 Sun ++1m>").unwrap();
        assert_eq!(d.repeater, Some("++1m".to_string()));

        let d = parse_org_date("<2026-05-10 Sun .+1y>").unwrap();
        assert_eq!(d.repeater, Some(".+1y".to_string()));
    }

    #[test]
    fn test_with_warning() {
        let d = parse_org_date("<2026-05-10 Sun -3d>").unwrap();
        assert_eq!(d.warning, Some("-3d".to_string()));
    }

    #[test]
    fn test_combined() {
        let d = parse_org_date("<2026-05-10 Sun 14:00 +1w -3d>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(14, 0, 0).unwrap());
        assert_eq!(d.repeater, Some("+1w".to_string()));
        assert_eq!(d.warning, Some("-3d".to_string()));
    }

    #[test]
    fn test_invalid_input() {
        assert!(parse_org_date("not a date").is_none());
        assert!(parse_org_date("<not-a-date>").is_none());
        assert!(parse_org_date("<2026-13-01>").is_none());
    }

    #[test]
    fn test_extract_timestamp() {
        let raw = "<2026-05-10 Sun>";
        assert_eq!(extract_timestamp(raw), Some(raw.to_string()));

        let raw = "SCHEDULED: <2026-05-10 Sun>";
        assert_eq!(extract_timestamp(raw), Some("<2026-05-10 Sun>".to_string()));
    }

    #[test]
    fn test_no_timestamp() {
        assert!(extract_timestamp("No timestamp here").is_none());
    }
}
