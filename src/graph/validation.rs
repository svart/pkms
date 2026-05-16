use super::{Graph, OverlinkEntry, SelfLinkEntry, resolve_file_link_path};
use crate::parser::Link;
use std::collections::HashMap;
use std::path::Path;

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
}
