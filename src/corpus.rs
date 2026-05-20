use crate::config::ResolvedConfig;
use crate::discovery::discover_files;
use crate::parser::{ParsedNote, parse_note};
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

#[derive(Debug, Clone)]
pub struct Corpus {
    results: Vec<FileScanResult>,
}

impl Corpus {
    pub fn load(config: &ResolvedConfig) -> Result<Self> {
        let db_root = config.resolved_db_root();
        let ignore = config.resolve_ignore_patterns();
        Self::scan(db_root, &ignore)
    }

    pub fn scan(db_root: &Path, ignore: &[String]) -> Result<Self> {
        let files = discover_files(db_root, ignore)?;
        let results: Vec<FileScanResult> = files
            .into_par_iter()
            .map(|entry| {
                let path = entry.path.clone();
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let parsed = parse_note(&content);
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
                }
            })
            .collect();

        Ok(Corpus { results })
    }

    pub fn results(&self) -> &[FileScanResult] {
        &self.results
    }
}
