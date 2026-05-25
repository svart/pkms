use crate::output::{ALL_COLUMNS, Column};
use crate::util::{is_stdin_piped, read_stdin_ndjson};
use anyhow::Result;
use chrono::NaiveDate;
#[cfg(test)]
use chrono::NaiveDateTime;

pub fn comma_list(value: Option<&str>) -> Option<Vec<String>> {
    value.map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
}

pub fn parse_date(value: Option<&str>) -> Option<NaiveDate> {
    value.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

#[cfg(test)]
pub fn parse_datetime(value: Option<&str>) -> Option<NaiveDateTime> {
    value.and_then(|d| {
        NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M")
            .ok()
            .or_else(|| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .ok()
                    .map(|dt| dt.and_hms_opt(0, 0, 0).expect("midnight is valid"))
            })
    })
}

pub fn resolve_targets(target: &Option<String>, from_stdin: bool) -> Result<Vec<String>> {
    if from_stdin || (target.is_none() && is_stdin_piped()) {
        read_stdin_ndjson()
    } else if let Some(t) = target {
        Ok(vec![t.clone()])
    } else {
        anyhow::bail!(
            "No target specified and no stdin pipe detected. \
             Provide a target or use --from-stdin."
        )
    }
}

pub fn resolve_columns(cli_cols: Option<&str>, config_cols: &Option<Vec<String>>) -> Vec<Column> {
    if let Some(s) = cli_cols {
        let cols: Vec<Column> = s
            .split(',')
            .filter_map(|c| Column::from_str(c.trim()))
            .collect();
        if !cols.is_empty() {
            return cols;
        }
    }
    if let Some(names) = config_cols {
        let cols: Vec<Column> = names.iter().filter_map(|c| Column::from_str(c)).collect();
        if !cols.is_empty() {
            return cols;
        }
    }
    ALL_COLUMNS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_list_splits_and_trims_values() {
        assert_eq!(
            comma_list(Some("alpha, beta,gamma")),
            Some(vec![
                "alpha".to_string(),
                "beta".to_string(),
                "gamma".to_string()
            ])
        );
        assert_eq!(comma_list(None), None);
    }

    #[test]
    fn parse_date_accepts_iso_dates_only() {
        assert_eq!(
            parse_date(Some("2026-05-20")),
            Some(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap())
        );
        assert_eq!(parse_date(Some("2026-02-30")), None);
        assert_eq!(parse_date(Some("20.05.2026")), None);
        assert_eq!(parse_date(None), None);
    }

    #[test]
    fn parse_datetime_accepts_minute_precision_and_date_only_midnight() {
        assert_eq!(
            parse_datetime(Some("2026-05-20 14:35")),
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 20)
                    .unwrap()
                    .and_hms_opt(14, 35, 0)
                    .unwrap()
            )
        );
        assert_eq!(
            parse_datetime(Some("2026-05-20")),
            Some(
                NaiveDate::from_ymd_opt(2026, 5, 20)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        );
        assert_eq!(parse_datetime(Some("2026-05-20 14:35:10")), None);
        assert_eq!(parse_datetime(Some("not-a-date")), None);
    }

    #[test]
    fn resolve_columns_prefers_valid_cli_columns_over_config() {
        let config_cols = Some(vec!["State".to_string(), "Note".to_string()]);
        assert_eq!(
            resolve_columns(Some("id, HEADING"), &config_cols),
            vec![Column::Id, Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_uses_config_when_cli_is_missing_or_invalid() {
        let config_cols = Some(vec!["state".to_string(), "heading".to_string()]);
        assert_eq!(
            resolve_columns(None, &config_cols),
            vec![Column::State, Column::Heading]
        );
        assert_eq!(
            resolve_columns(Some("unknown"), &config_cols),
            vec![Column::State, Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_falls_back_to_all_columns_when_no_valid_column_is_configured() {
        let config_cols = Some(vec!["unknown".to_string()]);
        assert_eq!(resolve_columns(None, &config_cols), ALL_COLUMNS.to_vec());
    }
}
