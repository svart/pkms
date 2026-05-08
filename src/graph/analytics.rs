use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use crate::parser::Link;

use super::{Graph, GraphStats, HeadingBacklinkEntry};

impl Graph {
    fn primary_nodes(&self) -> Vec<&super::Node> {
        let mut seen = std::collections::HashSet::new();
        self.nodes
            .values()
            .filter(|n| seen.insert(&n.uuid))
            .collect()
    }

    pub fn hubs(&self, limit: usize) -> Vec<(&super::Node, usize)> {
        let mut degrees: Vec<(&super::Node, usize)> = self
            .primary_nodes()
            .iter()
            .map(|n| {
                let outgoing = n
                    .outgoing
                    .iter()
                    .filter(|l| matches!(l, Link::Internal(_)))
                    .count();
                let incoming = self.backlinks.get(&n.uuid).map_or(0, std::vec::Vec::len);
                (*n, outgoing + incoming)
            })
            .collect();
        degrees.sort_by_key(|b| std::cmp::Reverse(b.1));
        degrees.truncate(limit);
        degrees
    }

    pub fn directory_breakdown(&self, db_root: &Path) -> Vec<(String, usize)> {
        let mut dirs: HashMap<String, usize> = HashMap::new();
        for node in self.primary_nodes() {
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
        let mut seen = std::collections::HashSet::new();
        self.nodes
            .values()
            .filter(|n| seen.insert(&n.path))
            .filter_map(|n| n.path.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    pub fn notes_since(&self, days: u32) -> Vec<&super::Node> {
        let cutoff = SystemTime::now() - std::time::Duration::from_secs(u64::from(days) * 86400);
        self.primary_nodes()
            .into_iter()
            .filter(|n| {
                n.path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .is_some_and(|t| t >= cutoff)
            })
            .collect()
    }

    /// Returns heading UUIDs that receive incoming backlinks, with source note info.
    pub fn heading_backlinks(&self) -> Vec<HeadingBacklinkEntry> {
        let mut entries = Vec::new();
        for (heading_uuid, primary_uuid) in &self.heading_uuid_to_primary {
            if let Some(sources) = self.backlinks.get(heading_uuid)
                && !sources.is_empty()
            {
                let primary_node = self.nodes.get(primary_uuid);
                for source_uuid in sources {
                    let source_node = self.nodes.get(source_uuid);
                    entries.push(HeadingBacklinkEntry {
                        heading_uuid: heading_uuid.clone(),
                        heading_title: String::new(), // filled by caller from node
                        primary_title: primary_node.map(|n| n.title.clone()).unwrap_or_default(),
                        primary_uuid: primary_uuid.clone(),
                        source_uuid: source_uuid.clone(),
                        source_title: source_node.map(|n| n.title.clone()).unwrap_or_default(),
                    });
                }
            }
        }
        entries
    }

    pub fn orphan_nodes(&self) -> Vec<&super::Node> {
        self.primary_nodes()
            .into_iter()
            .filter(|n| {
                let has_outgoing = n.outgoing.iter().any(|l| matches!(l, Link::Internal(_)));
                let has_incoming = self.backlinks.get(&n.uuid).is_some_and(|b| !b.is_empty());
                !has_outgoing && !has_incoming
            })
            .collect()
    }

    pub fn stats(&self) -> GraphStats {
        let total_notes = self.path_to_uuid.len();
        let total_internal_links: usize = self
            .path_to_uuid
            .keys()
            .filter_map(|p| self.nodes.get(self.path_to_uuid.get(p)?))
            .flat_map(|n| &n.outgoing)
            .filter(|l| matches!(l, Link::Internal(_)))
            .count();
        let total_file_links: usize = self
            .path_to_uuid
            .keys()
            .filter_map(|p| self.nodes.get(self.path_to_uuid.get(p)?))
            .flat_map(|n| &n.outgoing)
            .filter(|l| matches!(l, Link::File(_)))
            .count();
        let total_url_links: usize = self
            .path_to_uuid
            .keys()
            .filter_map(|p| self.nodes.get(self.path_to_uuid.get(p)?))
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
