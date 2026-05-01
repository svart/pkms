use crate::config::Config;
use crate::discovery::discover_files;
use crate::parser::{parse_note, Link, ParsedNote};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    #[allow(dead_code)]
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub path: PathBuf,
    pub filetags: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    #[allow(dead_code)]
    pub content_hash: String,
    pub outgoing: Vec<Link>,
    pub headings_count: usize,
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
    pub backlinks: HashMap<String, Vec<String>>,
    pub broken_links: Vec<(String, String)>,
    pub parse_errors: Vec<(PathBuf, String)>,
    pub skipped_files: Vec<PathBuf>,
    pub duplicates: DuplicateInfo,
}

impl Graph {
    pub fn build(results: Vec<FileScanResult>) -> Self {
        let mut nodes = HashMap::new();
        let mut path_to_uuid = HashMap::new();
        let mut title_to_uuid: HashMap<String, Vec<String>> = HashMap::new();
        let mut parse_errors = Vec::new();
        let mut skipped_files = Vec::new();
        let mut uuid_to_outgoing: HashMap<String, Vec<Link>> = HashMap::new();
        let mut uuid_to_path: HashMap<String, PathBuf> = HashMap::new();
        let mut missing_titles = Vec::new();
        let mut seen_uuids: HashMap<String, PathBuf> = HashMap::new();
        let mut duplicate_uuids = Vec::new();
        let mut seen_titles: HashMap<String, PathBuf> = HashMap::new();
        let mut duplicate_titles = Vec::new();

        for result in results {
            if let Some(err) = result.parse_error {
                parse_errors.push((result.path.clone(), err));
                skipped_files.push(result.path);
                continue;
            }

            let parsed = result.parsed;
            let path = result.path;

            let Some(uuid) = parsed.uuid else {
                skipped_files.push(path);
                continue;
            };

            if let Some(existing) = seen_uuids.get(&uuid) {
                duplicate_uuids.push(DuplicateEntry {
                    value: uuid.clone(),
                    paths: vec![
                        existing.to_string_lossy().to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                });
            } else {
                seen_uuids.insert(uuid.clone(), path.clone());
            }

            let title = match parsed.title.clone() {
                Some(t) => t,
                None => {
                    missing_titles.push(path.clone());
                    path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                }
            };

            if let Some(existing) = seen_titles.get(&title) {
                duplicate_titles.push(DuplicateEntry {
                    value: title.clone(),
                    paths: vec![
                        existing.to_string_lossy().to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                });
            } else {
                seen_titles.insert(title.clone(), path.clone());
            }

            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
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
            path_to_uuid.insert(path.clone(), uuid.clone());
            title_to_uuid.entry(title).or_default().push(uuid.clone());
            uuid_to_outgoing.insert(uuid.clone(), parsed.outgoing);
            uuid_to_path.insert(uuid, path);
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
                        broken_links.push((source_uuid.clone(), target_uuid.clone()));
                    }
                }
            }
        }

        // Deduplicate broken links
        broken_links.sort();
        broken_links.dedup();

        Graph {
            nodes,
            path_to_uuid,
            title_to_uuid,
            backlinks,
            broken_links,
            parse_errors,
            skipped_files,
            duplicates: DuplicateInfo {
                duplicate_uuids,
                duplicate_titles,
                missing_titles: missing_titles.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            },
        }
    }

    pub fn load(config: &Config, db_cli: Option<&Path>, verbose: bool) -> anyhow::Result<Self> {
        let db_root = config.resolve_db_root(db_cli)?;
        let ignore = config.resolve_ignore_patterns();

        if verbose {
            eprintln!("Scanning: {}", db_root.display());
        }

        let files = discover_files(&db_root, &ignore)?;

        if verbose {
            eprintln!("Found {} .org files, parsing...", files.len());
        }

        let results: Vec<FileScanResult> = files
            .into_iter()
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
        for node in self.nodes.values() {
            if node.aliases.iter().any(|a| a == target) {
                return Some(node);
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn find_node_exact(&self, target: &str) -> Option<&Node> {
        self.nodes.get(target)
            .or_else(|| {
                let p = PathBuf::from(target);
                self.path_to_uuid.get(&p).and_then(|u| self.nodes.get(u))
            })
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

    pub fn find_shortest_path(&self, from: &str, to: &str, max_depth: Option<u32>) -> Option<Vec<String>> {
        let from_uuid = self.find_node(from)?.uuid.clone();
        let to_uuid = self.find_node(to)?.uuid.clone();

        if from_uuid == to_uuid {
            return Some(vec![from_uuid]);
        }

        let mut visited = HashSet::new();
        visited.insert(from_uuid.clone());
        let mut queue = VecDeque::new();
        queue.push_back((from_uuid.clone(), vec![from_uuid]));

        while let Some((current, path)) = queue.pop_front() {
            if max_depth.map_or(false, |md| path.len() as u32 > md) {
                continue;
            }

            // Traverse outgoing links
            if let Some(node) = self.nodes.get(&current) {
                for link in &node.outgoing {
                    if let Link::Internal(next) = link {
                        if next == &to_uuid {
                            let mut full = path.clone();
                            full.push(next.clone());
                            return Some(full);
                        }
                        if visited.insert(next.clone()) {
                            let mut new_path = path.clone();
                            new_path.push(next.clone());
                            queue.push_back((next.clone(), new_path));
                        }
                    }
                }
            }

            // Traverse incoming links (backlinks)
            if let Some(backlinks) = self.backlinks.get(&current) {
                for prev in backlinks {
                    if prev == &to_uuid {
                        let mut full = path.clone();
                        full.push(prev.clone());
                        return Some(full);
                    }
                    if visited.insert(prev.clone()) {
                        let mut new_path = path.clone();
                        new_path.push(prev.clone());
                        queue.push_back((prev.clone(), new_path));
                    }
                }
            }
        }

        None
    }

    pub fn collect_subgraph(&self, root: &str, max_depth: u32) -> Subgraph {
        let mut nodes_map: HashMap<String, Node> = HashMap::new();
        let mut edges = Vec::new();
        let mut visited = HashSet::new();
        let root_uuid = match self.find_node(root) {
            Some(n) => n.uuid.clone(),
            None => return Subgraph::empty(),
        };

        let mut current = vec![root_uuid.clone()];
        visited.insert(root_uuid.clone());

        if let Some(root_node) = self.nodes.get(&root_uuid) {
            nodes_map.insert(root_uuid.clone(), root_node.clone());
        }

        for _depth in 1..=max_depth {
            let mut next = Vec::new();

            for uid in &current {
                if let Some(node) = self.nodes.get(uid) {
                    for link in &node.outgoing {
                        if let Link::Internal(target) = link {
                            edges.push((uid.clone(), target.clone()));
                            if visited.insert(target.clone()) {
                                next.push(target.clone());
                                if let Some(target_node) = self.nodes.get(target) {
                                    nodes_map.insert(target.clone(), target_node.clone());
                                }
                            }
                        }
                    }
                }

                if let Some(back) = self.backlinks.get(uid) {
                    for buid in back {
                        edges.push((buid.clone(), uid.clone()));
                        if visited.insert(buid.clone()) {
                            next.push(buid.clone());
                            if let Some(back_node) = self.nodes.get(buid) {
                                nodes_map.insert(buid.clone(), back_node.clone());
                            }
                        }
                    }
                }
            }

            current = next;
            if current.is_empty() {
                break;
            }
        }

        // Deduplicate edges
        edges.sort();
        edges.dedup();

        let vertex_count = nodes_map.len();
        let edge_count = edges.len();
        let avg_order = if vertex_count > 0 {
            edge_count as f64 / vertex_count as f64
        } else {
            0.0
        };

        let mut sorted_nodes: Vec<&Node> = nodes_map.values().collect();
        sorted_nodes.sort_by(|a, b| a.uuid.cmp(&b.uuid));

        Subgraph {
            root_uuid,
            nodes: sorted_nodes.into_iter().map(|n| n.clone()).collect(),
            edges,
            vertex_count,
            edge_count,
            avg_vertex_order: avg_order,
        }
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

            for ref_ in &node.refs {
                let ref_lower = ref_.to_lowercase();
                if words.iter().any(|w| ref_lower.contains(*w)) {
                    score += 6.0;
                    matches.push(format!("ref: {}", ref_));
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

    #[allow(dead_code)]
    pub fn search_by_ref(&self, terms: &str) -> Vec<(&Node, String)> {
        let query = terms.to_lowercase();
        let mut results = Vec::new();
        for node in self.nodes.values() {
            for ref_ in &node.refs {
                if ref_.to_lowercase().contains(&query) {
                    results.push((node, ref_.clone()));
                }
            }
        }
        results
    }

    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for node in self.nodes.values() {
            for tag in &node.filetags {
                *tag_counts.entry(tag.clone()).or_default() += 1;
            }
        }
        let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        tags
    }

    pub fn notes_by_tag(&self, tag: &str) -> Vec<&Node> {
        self.nodes
            .values()
            .filter(|n| n.filetags.iter().any(|t| t == tag))
            .collect()
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
        let duplicate_uuid_count = self.duplicates.duplicate_uuids.len();
        let duplicate_title_count = self.duplicates.duplicate_titles.len();
        let missing_title_count = self.duplicates.missing_titles.len();

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
            duplicate_uuid_count,
            duplicate_title_count,
            missing_title_count,
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

    #[test]
    fn test_shortest_path() {
        let results = vec![
            make_note("a", "A", vec![Link::Internal("b".to_string())]),
            make_note("b", "B", vec![Link::Internal("c".to_string())]),
            make_note("c", "C", vec![]),
        ];
        let graph = Graph::build(results);
        let path = graph.find_shortest_path("a", "c", None);
        assert!(path.is_some());
        assert_eq!(path.unwrap(), vec!["a", "b", "c"]);
    }
}
