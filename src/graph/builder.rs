use std::collections::HashMap;

use crate::parser::Link;

use super::{DuplicateEntry, DuplicateInfo, FileScanResult, Graph, Node};

impl Graph {
    #[allow(clippy::too_many_lines)]
    pub fn build(results: Vec<FileScanResult>) -> Self {
        let mut nodes = HashMap::new();
        let mut path_to_uuid = HashMap::new();
        let mut title_to_uuid: HashMap<String, Vec<String>> = HashMap::new();
        let mut alias_to_uuid: HashMap<String, Vec<String>> = HashMap::new();
        let mut parse_errors = Vec::new();
        let mut skipped_files = Vec::new();
        let mut missing_titles = Vec::new();
        let mut seen_uuids: HashMap<String, std::path::PathBuf> = HashMap::new();
        let mut duplicate_uuids = Vec::new();
        let mut seen_titles: HashMap<String, std::path::PathBuf> = HashMap::new();
        let mut duplicate_titles = Vec::new();
        let mut uuid_to_outgoing: HashMap<String, Vec<Link>> = HashMap::new();

        for result in results {
            if let Some(err) = result.parse_error {
                parse_errors.push((result.path.clone(), err));
                skipped_files.push(result.path);
                continue;
            }

            let parsed = result.parsed;
            let path = result.path;

            let Some(uuid) = parsed.uuid.as_ref() else {
                skipped_files.push(path);
                continue;
            };

            if let Some(existing) = seen_uuids.get(uuid) {
                duplicate_uuids.push(DuplicateEntry {
                    value: uuid.clone(),
                    paths: vec![
                        existing.to_string_lossy().to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                });
                continue;
            }
            seen_uuids.insert(uuid.clone(), path.clone());

            let title = if let Some(t) = parsed.title.clone() {
                t
            } else {
                missing_titles.push(path.clone());
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
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

            for alias in &parsed.roam_aliases {
                alias_to_uuid
                    .entry(alias.clone())
                    .or_default()
                    .push(uuid.clone());
            }

            let node = Node::from_parsed(uuid.clone(), title.clone(), path.clone(), &parsed);

            nodes.insert(uuid.clone(), node);
            path_to_uuid.insert(path, uuid.clone());
            title_to_uuid.entry(title).or_default().push(uuid.clone());
            uuid_to_outgoing.insert(uuid.clone(), parsed.outgoing);
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

        broken_links.sort();
        broken_links.dedup();

        Graph {
            nodes,
            path_to_uuid,
            title_to_uuid,
            alias_to_uuid,
            backlinks,
            broken_links,
            parse_errors,
            skipped_files,
            duplicates: DuplicateInfo {
                duplicate_uuids,
                duplicate_titles,
                missing_titles: missing_titles
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            },
        }
    }
}
