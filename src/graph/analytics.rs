use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use crate::domain::NoteId;
#[cfg(feature = "ssh")]
use crate::link_check::is_ssh_file_target;
use crate::parser::Link;
use crate::parser::find_daily_file_date;

use super::{Graph, GraphStats};

impl Graph {
    fn primary_nodes(&self) -> Vec<&super::Node> {
        self.nodes
            .values()
            .filter(|node| !self.heading_uuid_to_primary.contains_key(&node.uuid))
            .collect()
    }

    pub fn hubs(&self, limit: usize) -> Vec<(&super::Node, usize)> {
        let degrees = self.authored_internal_degrees();
        let mut degrees: Vec<(&super::Node, usize)> = self
            .primary_nodes()
            .iter()
            .map(|n| {
                let (outgoing, incoming) = degrees.get(&n.uuid).copied().unwrap_or_default();
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
                .map(|p| p.display().to_string())
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

    pub fn orphan_nodes_including_dailies(&self) -> Vec<&super::Node> {
        let degrees = self.authored_internal_degrees();
        self.primary_nodes()
            .into_iter()
            .filter(|n| {
                let (outgoing, incoming) = degrees.get(&n.uuid).copied().unwrap_or_default();
                outgoing == 0 && incoming == 0
            })
            .collect()
    }

    pub fn orphan_nodes(&self) -> Vec<&super::Node> {
        self.orphan_nodes_including_dailies()
            .into_iter()
            .filter(|n| find_daily_file_date(&n.path).is_none())
            .collect()
    }

    pub fn stats(&self) -> GraphStats {
        let total_notes = self.primary_nodes().len();
        let total_internal_links: usize = self
            .results
            .iter()
            .flat_map(authored_links)
            .filter(|l| matches!(l, Link::Internal(_)))
            .count();
        let total_file_links: usize = self
            .results
            .iter()
            .flat_map(authored_links)
            .filter(|l| matches!(l, Link::File(_)))
            .count();
        let total_url_links: usize = self
            .results
            .iter()
            .flat_map(authored_links)
            .filter(|l| matches!(l, Link::Url(_)))
            .count();
        #[cfg(feature = "ssh")]
        let total_ssh_links: usize = self
            .results
            .iter()
            .flat_map(authored_links)
            .filter(|l| matches!(l, Link::File(target) if is_ssh_file_target(target)))
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
            #[cfg(feature = "ssh")]
            total_ssh_links,
            orphan_notes,
            broken_link_count,
            skipped_count,
            parse_error_count,
            duplicate_uuid_count,
            duplicate_title_count,
            missing_title_count,
        }
    }

    pub(crate) fn authored_internal_degrees(&self) -> HashMap<NoteId, (usize, usize)> {
        let mut degrees: HashMap<NoteId, (usize, usize)> = self
            .primary_nodes()
            .into_iter()
            .map(|node| (node.uuid.clone(), (0, 0)))
            .collect();

        for result in &self.results {
            let Some(source_uuid) = self.path_to_uuid.get(&result.path) else {
                continue;
            };
            if self.heading_uuid_to_primary.contains_key(source_uuid) {
                continue;
            }

            for link in authored_links(result) {
                let Link::Internal(target_uuid) = link else {
                    continue;
                };

                degrees.entry(source_uuid.clone()).or_default().0 += 1;
                if let Some(target_primary) = self.primary_uuid_for_node_uuid(target_uuid) {
                    degrees.entry(target_primary).or_default().1 += 1;
                }
            }
        }

        degrees
    }

    fn primary_uuid_for_node_uuid(&self, uuid: &NoteId) -> Option<NoteId> {
        if let Some(primary) = self.heading_uuid_to_primary.get(uuid) {
            return Some(primary.clone());
        }
        self.nodes
            .contains_key(uuid)
            .then_some(uuid.clone())
            .filter(|uuid| !self.heading_uuid_to_primary.contains_key(uuid))
    }
}

fn authored_links(result: &super::FileScanResult) -> impl Iterator<Item = &Link> {
    result.parsed.outgoing.iter().chain(
        result
            .parsed
            .headings
            .iter()
            .flat_map(|h| h.outgoing.iter()),
    )
}
