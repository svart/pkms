use crate::ScanConfig;
use crate::discovery::discover_files;
use crate::parser::{ParsedNote, parse_note, parse_note_with_todo_states};
use anyhow::Result;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileScanResult {
    pub path: PathBuf,
    pub parsed: ParsedNote,
    pub raw_content: Option<String>,
    pub parse_error: Option<String>,
}

impl FileScanResult {
    pub fn without_raw_content(&self) -> Self {
        Self {
            path: self.path.clone(),
            parsed: self.parsed.clone(),
            raw_content: None,
            parse_error: self.parse_error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Corpus {
    results: Vec<FileScanResult>,
}

impl Corpus {
    pub fn load_from(config: &ScanConfig) -> Result<Self> {
        let db_root = &config.db_root;
        let ignore = &config.ignore_patterns;
        tracing::debug!(
            db_root = %db_root.display(),
            ignore_pattern_count = ignore.len(),
            "loading corpus"
        );
        Self::scan_with_todo_states(db_root, ignore, &config.todo_states)
    }

    pub fn scan(db_root: &Path, ignore: &[String]) -> Result<Self> {
        Self::scan_with(db_root, ignore, parse_note)
    }

    pub fn scan_with_todo_states(
        db_root: &Path,
        ignore: &[String],
        todo_states: &[String],
    ) -> Result<Self> {
        Self::scan_with(db_root, ignore, |content| {
            parse_note_with_todo_states(content, todo_states)
        })
    }

    fn scan_with(
        db_root: &Path,
        ignore: &[String],
        parse: impl Fn(&str) -> ParsedNote + Sync,
    ) -> Result<Self> {
        let files = discover_files(db_root, ignore)?;
        let results: Vec<FileScanResult> = files
            .into_par_iter()
            .map(|path| match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let parsed = parse(&content);
                    FileScanResult {
                        path,
                        parsed,
                        raw_content: Some(content),
                        parse_error: None,
                    }
                }
                Err(e) => FileScanResult {
                    path,
                    parsed: ParsedNote::empty(),
                    raw_content: None,
                    parse_error: Some(format!("IO error: {e}")),
                },
            })
            .collect();

        let parse_error_count = results
            .iter()
            .filter(|result| result.parse_error.is_some())
            .count();
        tracing::debug!(
            db_root = %db_root.display(),
            file_count = results.len(),
            parse_error_count,
            "corpus loaded"
        );
        Ok(Corpus { results })
    }

    pub fn results(&self) -> &[FileScanResult] {
        &self.results
    }
}
