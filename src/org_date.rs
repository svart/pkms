use chrono::{NaiveDate, NaiveTime};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OrgDate {
    pub base_date: NaiveDate,
    pub has_time: bool,
    pub time: Option<NaiveTime>,
    pub time_end: Option<NaiveTime>,
    pub base_date_end: Option<NaiveDate>,
    pub inactive: bool,
    pub repeater: Option<String>,
    pub warning: Option<String>,
    pub raw: String,
}

const MAX_PARSE_DEPTH: u32 = 32;

pub fn parse_org_date(raw: &str) -> Option<OrgDate> {
    parse_org_date_depth(raw, 0)
}

fn parse_org_date_depth(raw: &str, depth: u32) -> Option<OrgDate> {
    if depth > MAX_PARSE_DEPTH {
        return None;
    }
    let trimmed = raw.trim();

    if let Some(pos) = trimmed.find(">--<") {
        let first_raw = &trimmed[..pos + 1];
        let second_raw = &trimmed[pos + 3..];
        let mut first = parse_org_date_depth(first_raw, depth + 1)?;
        if let Some(second) = parse_org_date_depth(second_raw, depth + 1) {
            first.base_date_end = Some(second.base_date);
            if second.has_time {
                first.time_end = second.time;
            }
            first.raw = raw.to_string();
        }
        return Some(first);
    }

    let (inactive, inner) = if trimmed.starts_with('<') && trimmed.ends_with('>') {
        (false, &trimmed[1..trimmed.len() - 1])
    } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
        (true, &trimmed[1..trimmed.len() - 1])
    } else {
        return None;
    };

    let inner = inner.trim();
    if inner.is_empty() {
        return None;
    }

    if inner.starts_with('%') {
        return None;
    }

    let tokens: Vec<&str> = inner.split_whitespace().collect();
    let base_date = NaiveDate::parse_from_str(tokens[0], "%Y-%m-%d").ok()?;

    let mut has_time = false;
    let mut time = None;
    let mut time_end = None;
    let mut repeater = None;
    let mut warning = None;

    for token in &tokens[1..] {
        if token.is_empty() {
            continue;
        }
        if token.contains(':') {
            has_time = true;
            if let Some(pos) = token.find('-') {
                let start = NaiveTime::parse_from_str(&token[..pos], "%H:%M").ok()?;
                let end = NaiveTime::parse_from_str(&token[pos + 1..], "%H:%M").ok()?;
                time = Some(start);
                time_end = Some(end);
            } else {
                time = Some(NaiveTime::parse_from_str(token, "%H:%M").ok()?);
            }
        } else if token.starts_with('+') || token.starts_with('.') {
            if !is_valid_repeater(token) {
                return None;
            }
            repeater = Some(token.to_string());
        } else if token.starts_with('-') {
            if !is_valid_warning(token) {
                return None;
            }
            warning = Some(token.to_string());
        }
    }

    Some(OrgDate {
        base_date,
        has_time,
        time,
        time_end,
        base_date_end: None,
        inactive,
        repeater,
        warning,
        raw: raw.to_string(),
    })
}

fn is_valid_repeater(s: &str) -> bool {
    let body = s
        .strip_prefix(".+")
        .or_else(|| s.strip_prefix("++"))
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s);
    if body.len() < 2 {
        return false;
    }
    let (num_str, unit) = body.split_at(body.len() - 1);
    if num_str.is_empty() {
        return false;
    }
    num_str.chars().all(|c| c.is_ascii_digit()) && matches!(unit, "h" | "d" | "w" | "m" | "y")
}

fn is_valid_warning(s: &str) -> bool {
    let body = s
        .strip_prefix("--")
        .or_else(|| s.strip_prefix('-'))
        .unwrap_or(s);
    if body.len() < 2 {
        return false;
    }
    let (num_str, unit) = body.split_at(body.len() - 1);
    if num_str.is_empty() {
        return false;
    }
    num_str.chars().all(|c| c.is_ascii_digit()) && matches!(unit, "d" | "w" | "m" | "y")
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
        assert!(d.time_end.is_none());
        assert!(!d.inactive);
        assert!(d.repeater.is_none());
        assert!(d.warning.is_none());
    }

    #[test]
    fn test_date_without_dayname() {
        let d = parse_org_date("<2026-05-10>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(!d.has_time);
        assert!(!d.inactive);
    }

    #[test]
    fn test_date_with_time() {
        let d = parse_org_date("<2026-05-10 Sun 14:00>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(14, 0, 0).unwrap());
        assert!(d.time_end.is_none());
    }

    #[test]
    fn test_time_range() {
        let d = parse_org_date("<2026-05-10 Sun 10:00-12:00>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 10).unwrap());
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(10, 0, 0).unwrap());
        assert_eq!(
            d.time_end.unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_inactive_timestamp() {
        let d = parse_org_date("[2006-11-01 Wed]").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2006, 11, 1).unwrap());
        assert!(d.inactive);
        assert!(!d.has_time);
    }

    #[test]
    fn test_inactive_with_time() {
        let d = parse_org_date("[2006-11-01 Wed 19:15]").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2006, 11, 1).unwrap());
        assert!(d.inactive);
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(19, 15, 0).unwrap());
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
    fn test_hourly_repeater() {
        let d = parse_org_date("<2026-05-10 Sun 12:30 +1h>").unwrap();
        assert_eq!(d.repeater, Some("+1h".to_string()));
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(12, 30, 0).unwrap());
    }

    #[test]
    fn test_with_warning() {
        let d = parse_org_date("<2026-05-10 Sun -3d>").unwrap();
        assert_eq!(d.warning, Some("-3d".to_string()));
    }

    #[test]
    fn test_warning_double_dash() {
        let d = parse_org_date("<2024-12-25 Sat --2d>").unwrap();
        assert_eq!(d.warning, Some("--2d".to_string()));
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
        assert!(parse_org_date("").is_none());
        assert!(parse_org_date("<>").is_none());
    }

    #[test]
    fn test_russian_day_name() {
        let d = parse_org_date("<2026-05-13 Ср 11:00>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 13).unwrap());
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(11, 0, 0).unwrap());
    }

    #[test]
    fn test_russian_day_name_with_repeater_and_warning() {
        let d = parse_org_date("<2026-05-11 Пн ++1w -0d>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 11).unwrap());
        assert!(!d.has_time);
        assert_eq!(d.repeater, Some("++1w".to_string()));
        assert_eq!(d.warning, Some("-0d".to_string()));
    }

    #[test]
    fn test_diary_style_skipped() {
        assert!(parse_org_date("<%%(diary-float t 4 2) 22:00-23:00>").is_none());
    }

    #[test]
    fn test_date_range_individual() {
        let d = parse_org_date("<2004-08-23 Mon>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2004, 8, 23).unwrap());
        assert!(!d.inactive);
    }

    #[test]
    fn test_invalid_repeater() {
        assert!(parse_org_date("<2026-05-10 Sun +>").is_none());
        assert!(parse_org_date("<2026-05-10 Sun +x>").is_none());
        assert!(parse_org_date("<2026-05-10 Sun .>").is_none());
        assert!(parse_org_date("<2026-05-10 Sun ++>").is_none());
    }

    #[test]
    fn test_invalid_warning() {
        assert!(parse_org_date("<2026-05-10 Sun ->").is_none());
        assert!(parse_org_date("<2026-05-10 Sun -x>").is_none());
        assert!(parse_org_date("<2026-05-10 Sun -->").is_none());
        assert!(parse_org_date("<2026-05-10 Sun --x>").is_none());
    }

    #[test]
    fn test_no_timestamp() {
        assert!(parse_org_date("No timestamp here").is_none());
    }

    #[test]
    fn test_date_range_two_dates() {
        let d = parse_org_date("<2026-05-13 Wed>--<2026-05-15 Fri>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 13).unwrap());
        assert_eq!(
            d.base_date_end,
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert!(!d.has_time);
        assert!(d.time.is_none());
        assert!(d.time_end.is_none());
    }

    #[test]
    fn test_date_range_with_times() {
        let d = parse_org_date("<2026-05-13 Wed 12:00>--<2026-05-15 Fri 14:30>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 13).unwrap());
        assert_eq!(
            d.base_date_end,
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert_eq!(
            d.time_end.unwrap(),
            NaiveTime::from_hms_opt(14, 30, 0).unwrap()
        );
    }

    #[test]
    fn test_date_range_end_without_time() {
        let d = parse_org_date("<2026-05-13 Wed 12:00>--<2026-05-15 Fri>").unwrap();
        assert_eq!(d.base_date, NaiveDate::from_ymd_opt(2026, 5, 13).unwrap());
        assert_eq!(
            d.base_date_end,
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert!(d.has_time);
        assert_eq!(d.time.unwrap(), NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert!(d.time_end.is_none());
    }
}
