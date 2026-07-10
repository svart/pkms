use crate::output::{ALL_COLUMNS, Column};
use crate::util::{is_stdin_piped, read_stdin_ndjson};
use anyhow::{Result, bail};
#[cfg(test)]
use chrono::{NaiveDate, NaiveDateTime};

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

pub fn resolve_columns(
    cli_cols: Option<&str>,
    config_cols: Option<&[String]>,
) -> Result<Vec<Column>> {
    if let Some(raw_cli_cols) = cli_cols
        && !columns_has_adjustment(raw_cli_cols)
    {
        return match ColumnSelection::parse(raw_cli_cols)? {
            ColumnSelection::Replace(columns) => Ok(columns),
            ColumnSelection::Adjust(_) => {
                unreachable!("non-adjustment column selection parsed as adjustment")
            }
        };
    }

    let base = resolve_base_columns(config_cols)?;

    let Some(raw_cli_cols) = cli_cols else {
        return Ok(base);
    };

    match ColumnSelection::parse(raw_cli_cols)? {
        ColumnSelection::Replace(_) => {
            unreachable!("replacement column selection was handled before config")
        }
        ColumnSelection::Adjust(adjustments) => apply_column_adjustments(base, adjustments),
    }
}

enum ColumnSelection {
    Replace(Vec<Column>),
    Adjust(Vec<ColumnAdjustment>),
}

impl ColumnSelection {
    fn parse(raw: &str) -> Result<Self> {
        let tokens = split_column_tokens(raw)?;
        if tokens
            .iter()
            .any(|token| token.starts_with('+') || token.starts_with('-'))
        {
            parse_column_adjustments(tokens).map(ColumnSelection::Adjust)
        } else {
            parse_column_names(tokens.iter().copied()).map(ColumnSelection::Replace)
        }
    }
}

enum ColumnAdjustment {
    Enable(Column),
    Disable(Column),
}

impl ColumnAdjustment {
    fn parse(token: &str) -> Result<Self> {
        let (op, name) = token.split_at(1);
        if name.trim().is_empty() {
            bail!("--columns adjustment '{token}' is missing a column name");
        }
        let column = parse_column_name(name)?;
        match op {
            "+" => Ok(ColumnAdjustment::Enable(column)),
            "-" => Ok(ColumnAdjustment::Disable(column)),
            _ => unreachable!("adjustment operator was validated above"),
        }
    }
}

pub fn columns_has_adjustment(raw: &str) -> bool {
    raw.split(',')
        .map(str::trim)
        .any(|token| token.starts_with('+') || token.starts_with('-'))
}

fn split_column_tokens(raw: &str) -> Result<Vec<&str>> {
    let tokens: Vec<_> = raw
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        bail!("--columns must specify at least one column");
    }
    Ok(tokens)
}

fn resolve_base_columns(config_cols: Option<&[String]>) -> Result<Vec<Column>> {
    match config_cols {
        Some(names) => parse_column_names(names.iter().map(String::as_str)),
        None => Ok(ALL_COLUMNS.to_vec()),
    }
}

fn parse_column_adjustments(tokens: Vec<&str>) -> Result<Vec<ColumnAdjustment>> {
    if tokens
        .iter()
        .any(|token| !(token.starts_with('+') || token.starts_with('-')))
    {
        bail!("--columns cannot mix replacement columns with + or - adjustments");
    }

    tokens.into_iter().map(ColumnAdjustment::parse).collect()
}

fn apply_column_adjustments(
    mut columns: Vec<Column>,
    adjustments: Vec<ColumnAdjustment>,
) -> Result<Vec<Column>> {
    for adjustment in adjustments {
        match adjustment {
            ColumnAdjustment::Enable(column) => {
                if columns.contains(&column) {
                    bail!("Column '{}' is already enabled", column.name());
                }
                columns.push(column);
            }
            ColumnAdjustment::Disable(column) => {
                let Some(index) = columns.iter().position(|existing| *existing == column) else {
                    bail!("Column '{}' is not enabled", column.name());
                };
                columns.remove(index);
            }
        }
    }

    if columns.is_empty() {
        bail!("--columns removed all columns");
    }
    Ok(columns)
}

fn parse_column_names<'a>(names: impl Iterator<Item = &'a str>) -> Result<Vec<Column>> {
    let mut columns = Vec::new();
    for name in names {
        let column = parse_column_name(name)?;
        if columns.contains(&column) {
            bail!("Column '{}' is specified more than once", column.name());
        }
        columns.push(column);
    }
    if columns.is_empty() {
        bail!("At least one column must be configured");
    }
    Ok(columns)
}

fn parse_column_name(name: &str) -> Result<Column> {
    let trimmed = name.trim();
    trimmed.parse::<Column>().map_err(|_| {
        anyhow::anyhow!(
            "Unknown column '{}'. Available columns: {}",
            trimmed,
            ALL_COLUMNS
                .iter()
                .map(Column::name)
                .collect::<Vec<_>>()
                .join(",")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            resolve_columns(Some("id, HEADING"), config_cols.as_deref()).unwrap(),
            vec![Column::Id, Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_uses_config_when_cli_is_missing() {
        let config_cols = Some(vec!["state".to_string(), "heading".to_string()]);
        assert_eq!(
            resolve_columns(None, config_cols.as_deref()).unwrap(),
            vec![Column::State, Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_errors_on_unknown_columns() {
        let config_cols = Some(vec!["unknown".to_string()]);
        assert!(resolve_columns(None, config_cols.as_deref()).is_err());
        assert!(resolve_columns(Some("unknown"), None).is_err());
    }

    #[test]
    fn resolve_columns_exact_cli_overrides_invalid_config() {
        let config_cols = Some(vec!["unknown".to_string()]);
        assert_eq!(
            resolve_columns(Some("id,heading"), config_cols.as_deref()).unwrap(),
            vec![Column::Id, Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_adjusts_current_column_set() {
        let config_cols = Some(vec!["id".to_string(), "heading".to_string()]);
        assert_eq!(
            resolve_columns(Some("+project"), config_cols.as_deref()).unwrap(),
            vec![Column::Id, Column::Heading, Column::Project]
        );
        assert_eq!(
            resolve_columns(Some("-id"), config_cols.as_deref()).unwrap(),
            vec![Column::Heading]
        );
    }

    #[test]
    fn resolve_columns_rejects_ambiguous_adjustments() {
        let config_cols = Some(vec!["id".to_string(), "heading".to_string()]);
        assert!(resolve_columns(Some("+project,heading"), config_cols.as_deref()).is_err());
        assert!(resolve_columns(Some("+id"), config_cols.as_deref()).is_err());
        assert!(resolve_columns(Some("-project"), config_cols.as_deref()).is_err());
    }
}
