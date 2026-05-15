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
    pub(crate) results: Vec<FileScanResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfLinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub link_type: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverlinkEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
    pub target_title: String,
    pub count: usize,
}

pub fn resolve_file_link_path(target_path: &str, source_path: &Path, db_root: &Path) -> PathBuf {
    let (inner_path, is_org) = if let Some(rest) = target_path.strip_prefix("org:") {
        (rest, true)
    } else {
        (target_path, false)
    };
    let expanded = if inner_path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            inner_path.replacen('~', &home.to_string_lossy(), 1)
        } else {
            inner_path.to_string()
        }
    } else {
        inner_path.to_string()
    };
    let clean = expanded.split("::").next().unwrap_or(&expanded).to_string();
    let path = Path::new(&clean);
    if path.is_absolute() {
        path.to_path_buf()
    } else if is_org {
        db_root.join(path)
    } else {
        source_path.parent().unwrap_or(db_root).join(path)
    }
}

impl Graph {
    pub fn detect_self_links(&self, db_root: &Path) -> Vec<SelfLinkEntry> {
        let mut results = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        for path in self.path_to_uuid.keys() {
            if !seen_paths.insert(path.clone()) {
                continue;
            }
            let Some(primary_uuid) = self.path_to_uuid.get(path) else {
                continue;
            };

            let has_headings = self
                .nodes
                .values()
                .any(|n| n.path == *path && n.uuid != *primary_uuid);

            for node in self.nodes.values() {
                if node.path != *path {
                    continue;
                }
                for link in &node.outgoing {
                    match link {
                        Link::Internal(uuid) if uuid == &node.uuid => {
                            results.push(SelfLinkEntry {
                                source_uuid: node.uuid.clone(),
                                source_title: node.title.clone(),
                                link_type: "id".to_string(),
                                target: uuid.clone(),
                                suggestion: None,
                            });
                        }
                        Link::File(target_path) => {
                            let resolved = resolve_file_link_path(target_path, &node.path, db_root);
                            if resolved == *path {
                                let suggestion = if has_headings {
                                    Some(format!(
                                        "Use id:{} instead of file link to this file",
                                        primary_uuid
                                    ))
                                } else {
                                    None
                                };
                                results.push(SelfLinkEntry {
                                    source_uuid: node.uuid.clone(),
                                    source_title: node.title.clone(),
                                    link_type: "file".to_string(),
                                    target: target_path.clone(),
                                    suggestion,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        results
    }

    pub fn detect_overlinks(&self) -> Vec<OverlinkEntry> {
        let mut results = Vec::new();
        for node in self.nodes.values() {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for link in &node.outgoing {
                if let Link::Internal(target) = link {
                    *counts.entry(target.clone()).or_default() += 1;
                }
            }
            for (target_uuid, count) in counts {
                if count >= 2 {
                    let target_title = self
                        .nodes
                        .get(&target_uuid)
                        .map(|n| n.title.clone())
                        .unwrap_or_default();
                    results.push(OverlinkEntry {
                        source_uuid: node.uuid.clone(),
                        source_title: node.title.clone(),
                        target_uuid,
                        target_title,
                        count,
                    });
                }
            }
        }
        results
    }

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

    pub fn load(config: &Config) -> anyhow::Result<Self> {
        let db_root = config.resolved_db_root()?;
        let ignore = config.resolve_ignore_patterns();
        let results = Self::scan(db_root, &ignore)?;
        let mut graph = Graph::build(results.clone());
        graph.results = results;
        Ok(graph)
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

    pub fn all_task_entries(&self, config: &Config) -> Vec<(usize, String, usize)> {
        let entries = self.sorted_task_entries(config);
        entries
            .into_iter()
            .enumerate()
            .map(|(i, (p, l, _))| (i + 1, p, l))
            .collect()
    }

    pub fn resolve_canonical_task_id(
        &self,
        config: &Config,
        id: usize,
    ) -> anyhow::Result<(String, usize)> {
        let entries = self.sorted_task_entries(config);
        if id == 0 || id > entries.len() {
            anyhow::bail!(
                "No task with canonical ID {}. Valid range is 1–{}",
                id,
                entries.len()
            );
        }
        let (path, line, _) = &entries[id - 1];
        Ok((path.clone(), *line))
    }

    fn sorted_task_entries(&self, config: &Config) -> Vec<(String, usize, Option<char>)> {
        let valid_states = config.todo_states();
        let mut items: Vec<(String, usize, Option<char>)> = Vec::new();

        for result in &self.results {
            if result.parse_error.is_some() {
                continue;
            }
            for heading in &result.parsed.headings {
                let is_todo = heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)));
                let has_dates = heading.scheduled.is_some() || heading.deadline.is_some();
                if !is_todo && !has_dates {
                    continue;
                }
                items.push((
                    result.path.to_string_lossy().to_string(),
                    heading.line_number,
                    heading.priority,
                ));
            }
        }

        items.sort_by(|a, b| {
            let a_p = a.2.map(canonical_priority_value).unwrap_or(3);
            let b_p = b.2.map(canonical_priority_value).unwrap_or(3);
            a_p.cmp(&b_p).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1))
        });

        items
    }
}

fn canonical_priority_value(p: char) -> u8 {
    match p {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
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
