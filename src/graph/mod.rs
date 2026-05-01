pub mod analytics;
pub mod builder;
pub mod search;
pub mod traversal;

use crate::config::Config;
use crate::discovery::discover_files;
use crate::parser::{parse_note, Link, ParsedNote};
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
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings_count: usize,
}

impl Node {
    pub fn from_parsed(uuid: String, title: String, path: PathBuf, parsed: &ParsedNote) -> Self {
        Node {
            uuid,
            title,
            path,
            filetags: parsed.filetags.clone(),
            aliases: parsed.roam_aliases.clone(),
            refs: parsed.roam_refs.clone(),
            outgoing: parsed.outgoing.clone(),
            headings_count: parsed.headings.len(),
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
    pub nodes: HashMap<String, Node>,
    pub path_to_uuid: HashMap<PathBuf, String>,
    pub title_to_uuid: HashMap<String, Vec<String>>,
    pub alias_to_uuid: HashMap<String, Vec<String>>,
    pub backlinks: HashMap<String, Vec<String>>,
    pub broken_links: Vec<(String, String)>,
    pub parse_errors: Vec<(PathBuf, String)>,
    pub skipped_files: Vec<PathBuf>,
    pub duplicates: DuplicateInfo,
}

impl Graph {
    pub fn scan(db_root: &Path, ignore: &[String], verbose: bool) -> anyhow::Result<Vec<FileScanResult>> {
        if verbose {
            eprintln!("Scanning: {}", db_root.display());
        }

        let files = discover_files(db_root, ignore)?;

        if verbose {
            eprintln!("Found {} .org files, parsing...", files.len());
        }

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
                        parse_error: Some(format!("IO error: {}", e)),
                    },
                }
            })
            .collect();

        Ok(results)
    }

    pub fn load(config: &Config, db_cli: Option<&Path>, verbose: bool) -> anyhow::Result<Self> {
        let db_root = config.resolve_db_root(db_cli)?;
        let ignore = config.resolve_ignore_patterns();
        let results = Self::scan(&db_root, &ignore, verbose)?;
        Ok(Graph::build(results))
    }

    pub fn find_node(&self, target: &str) -> Option<&Node> {
        if let Some(node) = self.nodes.get(target) {
            return Some(node);
        }
        if let Some(uuid) = self.path_to_uuid.get(&PathBuf::from(target)) {
            return self.nodes.get(uuid);
        }
        if let Some(uuids) = self.title_to_uuid.get(target) {
            if let Some(uuid) = uuids.first() {
                return self.nodes.get(uuid);
            }
        }
        if let Some(uuids) = self.alias_to_uuid.get(target) {
            if let Some(uuid) = uuids.first() {
                return self.nodes.get(uuid);
            }
        }
        None
    }

    pub fn resolve_target(&self, target: &str) -> anyhow::Result<&Node> {
        self.find_node(target)
            .ok_or_else(|| anyhow::anyhow!("Note not found: {}", target))
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

#[derive(Debug, Clone, Serialize)]
pub struct Subgraph {
    pub root_uuid: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<(String, String)>,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub avg_vertex_order: f64,
}

impl Subgraph {
    pub fn empty() -> Self {
        Subgraph {
            root_uuid: String::new(),
            nodes: vec![],
            edges: vec![],
            vertex_count: 0,
            edge_count: 0,
            avg_vertex_order: 0.0,
        }
    }
}

#[cfg(test)]
mod tests;
