pub mod analytics;
pub mod builder;
pub mod search;
pub mod traversal;

use crate::config::Config;
use crate::discovery::discover_files;
use crate::parser::{Link, ParsedNote, parse_note};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub uuid: String,
    pub title: String,
    pub path: PathBuf,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings_count: usize,
    pub heading_uuids: Vec<String>,
    pub has_todos: bool,
}

impl Node {
    pub fn from_parsed(uuid: String, title: String, path: PathBuf, parsed: &ParsedNote) -> Self {
        Node {
            uuid,
            title,
            path,
            filetags: parsed.filetags.clone(),
            categories: parsed.categories.clone(),
            aliases: parsed.roam_aliases.clone(),
            refs: parsed.roam_refs.clone(),
            outgoing: parsed.outgoing.clone(),
            headings_count: parsed.headings.len(),
            heading_uuids: parsed.heading_uuids(),
            has_todos: parsed.has_todo_headings(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileScanResult {
    pub path: PathBuf,
    pub parsed: ParsedNote,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateInfo {
    pub duplicate_uuids: Vec<DuplicateEntry>,
    pub duplicate_titles: Vec<DuplicateEntry>,
    pub missing_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateEntry {
    pub value: String,
    pub paths: Vec<String>,
}

#[derive(Debug)]
pub struct Graph {
    pub(crate) nodes: HashMap<String, Node>,
    pub(crate) path_to_uuid: HashMap<PathBuf, String>,
    pub(crate) title_to_uuid: HashMap<String, Vec<String>>,
    pub(crate) alias_to_uuid: HashMap<String, Vec<String>>,
    pub(crate) backlinks: HashMap<String, Vec<String>>,
    pub(crate) broken_links: Vec<(String, String)>,
    pub(crate) parse_errors: Vec<(PathBuf, String)>,
    pub(crate) skipped_files: Vec<PathBuf>,
    pub(crate) duplicates: DuplicateInfo,
    pub(crate) heading_uuid_to_primary: HashMap<String, String>,
}

impl Graph {
    pub fn scan(db_root: &Path, ignore: &[String]) -> anyhow::Result<Vec<FileScanResult>> {
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
                            parse_error: None,
                        }
                    }
                    Err(e) => FileScanResult {
                        path,
                        parsed: ParsedNote::empty(),
                        parse_error: Some(format!("IO error: {e}")),
                    },
                }
            })
            .collect();

        Ok(results)
    }

    pub fn load(config: &Config, db_cli: Option<&Path>) -> anyhow::Result<Self> {
        let db_root = config.resolve_db_root(db_cli)?;
        let ignore = config.resolve_ignore_patterns();
        let results = Self::scan(&db_root, &ignore)?;
        Ok(Graph::build(results))
    }

    pub fn find_node(&self, target: &str) -> Option<&Node> {
        if let Some(node) = self.nodes.get(target) {
            return Some(node);
        }
        if let Some(uuid) = self.path_to_uuid.get(&PathBuf::from(target)) {
            return self.nodes.get(uuid);
        }
        if let Some(uuids) = self.title_to_uuid.get(target)
            && let Some(uuid) = uuids.first()
        {
            return self.nodes.get(uuid);
        }
        if let Some(uuids) = self.alias_to_uuid.get(target)
            && let Some(uuid) = uuids.first()
        {
            return self.nodes.get(uuid);
        }
        if let Some(primary) = self.heading_uuid_to_primary.get(target) {
            return self.nodes.get(primary);
        }
        None
    }

    pub fn resolve_target(&self, target: &str) -> anyhow::Result<&Node> {
        self.find_node(target)
            .ok_or_else(|| anyhow::anyhow!("Note not found: {target}"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct NeighborSet {
    pub outgoing: Vec<Node>,
    pub incoming: Vec<Node>,
    pub broken_outgoing: Vec<Link>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    pub total_notes: usize,
    pub total_links: usize,
    pub total_internal_links: usize,
    pub total_file_links: usize,
    pub total_url_links: usize,
    pub orphan_notes: usize,
    pub broken_link_count: usize,
    pub skipped_count: usize,
    pub parse_error_count: usize,
    pub duplicate_uuid_count: usize,
    pub duplicate_title_count: usize,
    pub missing_title_count: usize,
}

#[cfg(test)]
mod tests;
