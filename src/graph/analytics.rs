use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use crate::parser::Link;

use super::{Graph, GraphStats};

impl Graph {
    pub fn hubs(&self, limit: usize) -> Vec<(&super::Node, usize)> {
        let mut degrees: Vec<(&super::Node, usize)> = self
            .nodes
            .values()
            .map(|n| {
                let outgoing = n
                    .outgoing
                    .iter()
                    .filter(|l| matches!(l, Link::Internal(_)))
                    .count();
                let incoming = self.backlinks.get(&n.uuid).map_or(0, std::vec::Vec::len);
                (n, outgoing + incoming)
            })
            .collect();
        degrees.sort_by_key(|b| std::cmp::Reverse(b.1));
        degrees.truncate(limit);
        degrees
    }

    pub fn directory_breakdown(&self, db_root: &Path) -> Vec<(String, usize)> {
        let mut dirs: HashMap<String, usize> = HashMap::new();
        for node in self.nodes.values() {
            let rel = node.path.strip_prefix(db_root).unwrap_or(&node.path);
            let dir = rel
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            *dirs.entry(dir).or_default() += 1;
        }
        let mut result: Vec<_> = dirs.into_iter().collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.1));
        result
    }

    pub fn disk_size(&self) -> u64 {
        self.nodes
            .values()
            .filter_map(|n| n.path.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    pub fn notes_since(&self, days: u32) -> Vec<&super::Node> {
        let cutoff = SystemTime::now() - std::time::Duration::from_secs(u64::from(days) * 86400);
        self.nodes
            .values()
            .filter(|n| {
                n.path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .is_some_and(|t| t >= cutoff)
            })
            .collect()
    }

    pub fn orphan_nodes(&self) -> Vec<&super::Node> {
        self.nodes
            .values()
            .filter(|n| {
                let has_outgoing = n.outgoing.iter().any(|l| matches!(l, Link::Internal(_)));
                let has_incoming = self.backlinks.get(&n.uuid).is_some_and(|b| !b.is_empty());
                !has_outgoing && !has_incoming
            })
            .collect()
    }

    pub fn broken_links_list(&self) -> Vec<(String, String, String)> {
        self.broken_links
            .iter()
            .map(|(src, tgt)| {
                let title = self
                    .nodes
                    .get(src)
                    .map(|n| n.title.clone())
                    .unwrap_or_default();
                (src.clone(), title, tgt.clone())
            })
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
        let orphan_notes = self.orphan_nodes().len();
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
