use std::collections::HashMap;

use crate::parser::Link;

use super::{DuplicateEntry, DuplicateInfo, FileScanResult, Graph, Node};

type Backlinks = HashMap<String, Vec<String>>;
type BrokenLinks = Vec<(String, String)>;

fn build_links(
    uuid_to_outgoing: &HashMap<String, Vec<Link>>,
    nodes: &HashMap<String, Node>,
) -> (Backlinks, BrokenLinks) {
    let mut backlinks: Backlinks = HashMap::new();
    let mut broken_links = BrokenLinks::new();

    for (source_uuid, links) in uuid_to_outgoing {
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

    (backlinks, broken_links)
}

impl Graph {
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
        let mut heading_uuid_to_primary: HashMap<String, String> = HashMap::new();
        let mut all_uuids_seen: HashMap<String, std::path::PathBuf> = HashMap::new();

        for result in results {
            if let Some(err) = result.parse_error {
                parse_errors.push((result.path.clone(), err));
                skipped_files.push(result.path);
                continue;
            }

            let parsed = result.parsed;
            let path = result.path;

            if parsed.uuids.is_empty() {
                skipped_files.push(path);
                continue;
            }

            let primary_uuid = &parsed.uuids[0];

            let is_duplicate = seen_uuids
                .get(primary_uuid.as_str())
                .is_some_and(|existing| existing != &path);
            if is_duplicate {
                duplicate_uuids.push(DuplicateEntry {
                    value: primary_uuid.clone(),
                    paths: vec![
                        seen_uuids[primary_uuid.as_str()]
                            .to_string_lossy()
                            .to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                });
                continue;
            }

            // Check primary UUID against all_uuids_seen (which has heading UUIDs from other files)
            if let Some(existing) = all_uuids_seen.get(primary_uuid.as_str())
                && existing != &path
            {
                duplicate_uuids.push(DuplicateEntry {
                    value: primary_uuid.clone(),
                    paths: vec![
                        existing.to_string_lossy().to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                });
            }

            seen_uuids.insert(primary_uuid.clone(), path.clone());

            // Track primary UUIDs in all_uuids_seen too, so heading UUIDs
            // can be checked against primary UUIDs from other files
            all_uuids_seen.insert(primary_uuid.clone(), path.clone());

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
                    .push(primary_uuid.clone());
            }

            let node =
                Node::from_parsed(primary_uuid.clone(), title.clone(), path.clone(), &parsed);

            // Map heading UUIDs to primary UUID for resolution
            for heading_uuid in &node.heading_uuids {
                heading_uuid_to_primary
                    .entry(heading_uuid.clone())
                    .or_insert_with(|| primary_uuid.clone());
            }

            // Check heading UUIDs for uniqueness against ALL seen UUIDs
            let mut file_heading_uuids_seen = std::collections::HashSet::new();
            for heading_uuid in &node.heading_uuids {
                // Intra-file duplicate: heading UUID matches own primary UUID
                if heading_uuid == primary_uuid {
                    duplicate_uuids.push(DuplicateEntry {
                        value: heading_uuid.clone(),
                        paths: vec![
                            path.to_string_lossy().to_string(),
                            path.to_string_lossy().to_string(),
                        ],
                    });
                    continue;
                }
                // Intra-file duplicate: same heading UUID appears in two headings
                if !file_heading_uuids_seen.insert(heading_uuid.clone()) {
                    duplicate_uuids.push(DuplicateEntry {
                        value: heading_uuid.clone(),
                        paths: vec![
                            path.to_string_lossy().to_string(),
                            path.to_string_lossy().to_string(),
                        ],
                    });
                    continue;
                }
                if let Some(existing) = all_uuids_seen.get(heading_uuid.as_str())
                    && existing != &path
                {
                    duplicate_uuids.push(DuplicateEntry {
                        value: heading_uuid.clone(),
                        paths: vec![
                            existing.to_string_lossy().to_string(),
                            path.to_string_lossy().to_string(),
                        ],
                    });
                }
                all_uuids_seen.insert(heading_uuid.clone(), path.clone());
            }

            // Insert primary UUID into nodes
            nodes.insert(primary_uuid.clone(), node.clone());

            // Also insert heading UUIDs pointing to the same node for link resolution
            for heading_uuid in &node.heading_uuids {
                nodes
                    .entry(heading_uuid.clone())
                    .or_insert_with(|| node.clone());
            }

            path_to_uuid.insert(path, primary_uuid.clone());
            title_to_uuid
                .entry(title)
                .or_default()
                .push(primary_uuid.clone());
            uuid_to_outgoing.insert(primary_uuid.clone(), parsed.outgoing);
        }

        let (backlinks, broken_links) = build_links(&uuid_to_outgoing, &nodes);

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
            heading_uuid_to_primary,
        }
    }
}
