use crate::output::{ALL_COLUMNS, Column};
use crate::util::{is_stdin_piped, read_stdin_ndjson};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime};

pub fn comma_list(value: Option<&str>) -> Option<Vec<String>> {
    value.map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
}

pub fn parse_date(value: Option<&str>) -> Option<NaiveDate> {
    value.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

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
