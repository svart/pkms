use crate::parser::{Link, ParsedNote};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
pub struct Node {
    #[allow(dead_code)]
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub path: PathBuf,
    pub filetags: Vec<String>,
    #[allow(dead_code)]
    pub aliases: Vec<String>,
    #[allow(dead_code)]
    pub refs: Vec<String>,
    #[allow(dead_code)]
    pub content_hash: String,
    pub outgoing: Vec<Link>,
    #[allow(dead_code)]
    pub headings_count: usize,
}

#[derive(Debug, Clone)]
pub struct FileScanResult {
    pub path: PathBuf,
    pub parsed: ParsedNote,
    pub parse_error: Option<String>,
}

#[derive(Debug)]
pub struct Graph {
    pub nodes: HashMap<String, Node>,
    pub path_to_uuid: HashMap<PathBuf, String>,
    pub title_to_uuid: HashMap<String, Vec<String>>,
    pub backlinks: HashMap<String, Vec<String>>,
    pub broken_links: Vec<(String, String)>,
    pub parse_errors: Vec<(PathBuf, String)>,
    pub skipped_files: Vec<PathBuf>,
}

impl Graph {
    pub fn build(results: Vec<FileScanResult>) -> Self {
        let mut nodes = HashMap::new();
        let mut path_to_uuid = HashMap::new();
        let mut title_to_uuid: HashMap<String, Vec<String>> = HashMap::new();
        let mut parse_errors = Vec::new();
        let mut skipped_files = Vec::new();
        let mut uuid_to_outgoing: HashMap<String, Vec<Link>> = HashMap::new();

        for result in results {
            if let Some(err) = result.parse_error {
                parse_errors.push((result.path.clone(), err));
                skipped_files.push(result.path);
                continue;
            }

            let parsed = result.parsed;
            let Some(uuid) = parsed.uuid else {
                skipped_files.push(result.path);
                continue;
            };

            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = result.path.clone();
            let title = parsed.title.clone().unwrap_or_else(|| {
                result
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });

            let headings_count = parsed.headings.len();

            let node = Node {
                id,
                uuid: uuid.clone(),
                title: title.clone(),
                path: path.clone(),
                filetags: parsed.filetags,
                aliases: parsed.roam_aliases,
                refs: parsed.roam_refs,
                content_hash: parsed.content_hash,
                outgoing: parsed.outgoing.clone(),
                headings_count,
            };

            nodes.insert(uuid.clone(), node);
            path_to_uuid.insert(path, uuid.clone());
            title_to_uuid.entry(title).or_default().push(uuid.clone());
            uuid_to_outgoing.insert(uuid, parsed.outgoing);
        }

        let mut backlinks: HashMap<String, Vec<String>> = HashMap::new();
        let mut broken_links = Vec::new();

        for (source_uuid, links) in &uuid_to_outgoing {
            for link in links {
                if let Link::Internal(target_uuid) = link {
                    if nodes.contains_key(target_uuid) {
                        backlinks
                            .entry(target_uuid.clone())
                            .or_default()
                            .push(source_uuid.clone());
                    } else {
                        broken_links
                            .push((source_uuid.clone(), target_uuid.clone()));
                    }
                }
            }
        }

        Graph {
            nodes,
            path_to_uuid,
            title_to_uuid,
            backlinks,
            broken_links,
            parse_errors,
            skipped_files,
        }
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
        for node in self.nodes.values() {
            if node.aliases.iter().any(|a| a == target) {
                return Some(node);
            }
        }
        None
    }

    pub fn get_neighbors(&self, uuid: &str, max_depth: u32) -> HashMap<u32, NeighborSet> {
        let mut result = HashMap::new();
        let mut visited = HashSet::new();
        visited.insert(uuid.to_string());

        let mut current = vec![uuid.to_string()];
        for depth in 1..=max_depth {
            let mut neighbors = NeighborSet::default();
            let mut next = Vec::new();

            for uid in &current {
                if let Some(node) = self.nodes.get(uid) {
                    for link in &node.outgoing {
                        if let Link::Internal(target) = link {
                            if visited.insert(target.clone()) {
                                next.push(target.clone());
                            }
                            if let Some(target_node) = self.nodes.get(target) {
                                neighbors.outgoing.push(target_node.clone());
                            } else {
                                neighbors.broken_outgoing.push(link.clone());
                            }
                        }
                    }
                }

                if let Some(backlinks) = self.backlinks.get(uid) {
                    for buid in backlinks {
                        if visited.insert(buid.clone()) {
                            next.push(buid.clone());
                        }
                        if let Some(back_node) = self.nodes.get(buid) {
                            neighbors.incoming.push(back_node.clone());
                        }
                    }
                }
            }

            // Deduplicate by UUID
            neighbors.outgoing.sort_by(|a, b| a.uuid.cmp(&b.uuid));
            neighbors.outgoing.dedup_by(|a, b| a.uuid == b.uuid);
            neighbors.incoming.sort_by(|a, b| a.uuid.cmp(&b.uuid));
            neighbors.incoming.dedup_by(|a, b| a.uuid == b.uuid);

            result.insert(depth, neighbors);
            current = next;
            if current.is_empty() {
                break;
            }
        }

        result
    }

    pub fn search(&self, terms: &str) -> Vec<(&Node, f64, Vec<String>)> {
        let query = terms.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut results: Vec<(&Node, f64, Vec<String>)> = Vec::new();

        for node in self.nodes.values() {
            let mut score = 0.0;
            let mut matches = Vec::new();

            let title_lower = node.title.to_lowercase();
            let words_in_title: usize = words.iter().filter(|w| title_lower.contains(*w)).count();
            if words_in_title > 0 {
                score += words_in_title as f64 * 10.0;
                matches.push(format!("title: {}", node.title));
            }

            for alias in &node.aliases {
                let alias_lower = alias.to_lowercase();
                let words_in_alias: usize =
                    words.iter().filter(|w| alias_lower.contains(*w)).count();
                if words_in_alias > 0 {
                    score += words_in_alias as f64 * 8.0;
                    matches.push(format!("alias: {}", alias));
                }
            }

            for tag in &node.filetags {
                if words.iter().any(|w| tag.contains(*w)) {
                    score += 5.0;
                    matches.push(format!("tag: {}", tag));
                }
            }

            if score > 0.0 {
                results.push((node, score, matches));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    pub fn search_content(&self, terms: &str) -> Vec<(String, String, Vec<String>)> {
        let query = terms.to_lowercase();
        let mut results = Vec::new();

        for node in self.nodes.values() {
            if let Ok(content) = std::fs::read_to_string(&node.path) {
                let content_lower = content.to_lowercase();
                if content_lower.contains(&query) {
                    let mut context_lines = Vec::new();
                    for (i, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(&query) {
                            context_lines.push(format!("{}: {}", i + 1, line.trim()));
                        }
                    }
                    results.push((node.uuid.clone(), node.title.clone(), context_lines));
                }
            }
        }

        results
    }

    pub fn stats(&self) -> GraphStats {
        let total_notes = self.nodes.len();
        let total_internal_links: usize = self
            .nodes
            .values()
            .flat_map(|n| &n.outgoing)
            .filter(|l| matches!(l, Link::Internal(_)))
            .count();
        let total_file_links: usize = self
            .nodes
            .values()
            .flat_map(|n| &n.outgoing)
            .filter(|l| matches!(l, Link::File(_)))
            .count();
        let total_url_links: usize = self
            .nodes
            .values()
            .flat_map(|n| &n.outgoing)
            .filter(|l| matches!(l, Link::Url(_)))
            .count();
        let total_links = total_internal_links + total_file_links + total_url_links;
        let orphan_notes = self
            .nodes
            .values()
            .filter(|n| {
                let has_outgoing = n.outgoing.iter().any(|l| matches!(l, Link::Internal(_)));
                let has_incoming = self
                    .backlinks
                    .get(&n.uuid)
                    .map_or(false, |b| !b.is_empty());
                !has_outgoing && !has_incoming
            })
            .count();
        let broken_link_count = self.broken_links.len();
        let skipped_count = self.skipped_files.len();
        let parse_error_count = self.parse_errors.len();

        GraphStats {
            total_notes,
            total_links,
            total_internal_links,
            total_file_links,
            total_url_links,
            orphan_notes,
            broken_link_count,
            skipped_count,
            parse_error_count,
        }
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
}

use serde::Serialize;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParsedNote;

    fn make_note(uuid: &str, title: &str, outgoing: Vec<Link>) -> FileScanResult {
        FileScanResult {
            path: PathBuf::from(format!("{}.org", uuid)),
            parsed: ParsedNote {
                uuid: Some(uuid.to_string()),
                title: Some(title.to_string()),
                filetags: vec![],
                roam_aliases: vec![],
                roam_refs: vec![],
                outgoing,
                headings: vec![],
                content_hash: "abc".to_string(),
            },
            parse_error: None,
        }
    }

    #[test]
    fn test_graph_build() {
        let results = vec![
            make_note("a", "Note A", vec![Link::Internal("b".to_string())]),
            make_note("b", "Note B", vec![]),
        ];
        let graph = Graph::build(results);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.broken_links.len(), 0);
        assert_eq!(graph.backlinks.get("b").unwrap().len(), 1);
    }

    #[test]
    fn test_broken_links() {
        let results = vec![make_note(
            "a",
            "Note A",
            vec![Link::Internal("nonexistent".to_string())],
        )];
        let graph = Graph::build(results);
        assert_eq!(graph.broken_links.len(), 1);
    }

    #[test]
    fn test_orphan_detection() {
        let results = vec![
            make_note("a", "Orphan", vec![]),
            make_note("b", "Connected", vec![Link::Internal("c".to_string())]),
            make_note("c", "Target", vec![]),
        ];
        let graph = Graph::build(results);
        let stats = graph.stats();
        assert_eq!(stats.orphan_notes, 1);
    }
}
