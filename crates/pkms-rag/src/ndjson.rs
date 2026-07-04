use std::{error::Error, fmt, fs, path::Path};

use anyhow::{Context, Result};

use crate::models::RetrievalRecord;

#[derive(Debug)]
pub struct NdjsonError {
    line_number: usize,
    source: serde_json::Error,
}

impl NdjsonError {
    pub fn line_number(&self) -> usize {
        self.line_number
    }
}

impl fmt::Display for NdjsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid NDJSON record on line {}: {}",
            self.line_number, self.source
        )
    }
}

impl Error for NdjsonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

pub fn parse_ndjson(text: &str) -> std::result::Result<Vec<RetrievalRecord>, NdjsonError> {
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let record = serde_json::from_str(stripped).map_err(|source| NdjsonError {
            line_number: index + 1,
            source,
        })?;
        records.push(record);
    }
    Ok(records)
}

pub fn load_ndjson(path: impl AsRef<Path>) -> Result<Vec<RetrievalRecord>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read NDJSON file {}", display_path(path)))?;
    parse_ndjson(&text)
        .with_context(|| format!("failed to parse NDJSON file {}", display_path(path)))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SUPPORTED_SCHEMA_VERSION;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retrieval-export.ndjson")
    }

    #[test]
    fn ndjson_parses_valid_fixture() {
        let records = load_ndjson(fixture_path()).expect("fixture parses");

        assert_eq!(records.len(), 5);
        assert!(matches!(records[0], RetrievalRecord::Note(_)));
        assert!(matches!(records[1], RetrievalRecord::Chunk(_)));
        assert!(matches!(records[4], RetrievalRecord::Link(_)));
    }

    #[test]
    fn ndjson_ignores_blank_lines() {
        let records = parse_ndjson(
            r#"

{"schema_version":1,"record_type":"delete","entity_type":"chunk","entity_id":"old:chunk"}

"#,
        )
        .expect("blank lines are ignored");

        assert_eq!(records.len(), 1);
        let RetrievalRecord::Delete(delete) = &records[0] else {
            panic!("expected delete record");
        };
        assert_eq!(delete.schema_version, SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn ndjson_reports_invalid_json_line_numbers() {
        let err = parse_ndjson(
            r#"
{"schema_version":1,"record_type":"delete","entity_type":"chunk","entity_id":"old:chunk"}
not json
"#,
        )
        .expect_err("invalid json is rejected");

        assert_eq!(err.line_number(), 3);
        assert!(err.to_string().contains("line 3"));
    }

    #[test]
    fn ndjson_reports_unsupported_schema_versions_with_line_numbers() {
        let err = parse_ndjson(
            r#"
{"schema_version":1,"record_type":"delete","entity_type":"chunk","entity_id":"old:chunk"}
{"schema_version":2,"record_type":"delete","entity_type":"chunk","entity_id":"old:chunk"}
"#,
        )
        .expect_err("unsupported schema version is rejected");

        assert_eq!(err.line_number(), 3);
        assert!(err.to_string().contains("unsupported schema_version 2"));
    }
}
