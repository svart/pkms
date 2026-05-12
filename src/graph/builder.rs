use std::collections::HashMap;
use std::path::Path;

use crate::parser::{Heading, Link};

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

#[allow(clippy::too_many_arguments)]
fn process_headings(
    headings: &[Heading],
    primary_uuid: &str,
    filetags: &[String],
    categories: &[String],
    aliases: &[String],
    refs: &[String],
    path: &Path,
    nodes: &mut HashMap<String, Node>,
    uuid_to_outgoing: &mut HashMap<String, Vec<Link>>,
    heading_uuid_to_primary: &mut HashMap<String, String>,
) {
    let mut stack: Vec<(usize, String, Vec<String>)> = Vec::new();

    for (i, heading) in headings.iter().enumerate() {
        while let Some(&(top_idx, _, _)) = stack.last() {
            if headings[top_idx].level >= heading.level {
                let (_, uuid, children) = stack.pop().unwrap();
                if let Some(node) = nodes.get_mut(&uuid) {
                    node.heading_uuids = children;
                    node.headings_count = node.heading_uuids.len();
                }
            } else {
                break;
            }
        }

        if let Some(uuid) = &heading.uuid {
            let parent_uuid = stack
                .last()
                .map(|(_, u, _)| u.clone())
                .unwrap_or_else(|| primary_uuid.to_string());

            let mut heading_node = Node {
                uuid: uuid.clone(),
                title: heading.title.clone(),
                path: path.to_path_buf(),
                filetags: {
                    let mut ft = filetags.to_vec();
                    ft.extend(heading.tags.clone());
                    ft
                },
                categories: categories.to_vec(),
                aliases: aliases.to_vec(),
                refs: refs.to_vec(),
                outgoing: {
                    let mut o = heading.outgoing.clone();
                    o.push(Link::Internal(parent_uuid.clone()));
                    o
                },
                headings_count: 0,
                heading_uuids: vec![],
                has_todos: heading.todo_state.is_some(),
            };

            uuid_to_outgoing.insert(uuid.clone(), {
                let mut o = heading.outgoing.clone();
                o.push(Link::Internal(parent_uuid.clone()));
                o
            });

            uuid_to_outgoing
                .entry(parent_uuid.clone())
                .or_default()
                .push(Link::Internal(uuid.clone()));

            heading_node.outgoing = uuid_to_outgoing[&uuid.clone()].clone();

            heading_uuid_to_primary.insert(uuid.clone(), primary_uuid.to_string());

            nodes.insert(uuid.clone(), heading_node);

            if let Some((_, _, children)) = stack.last_mut() {
                children.push(uuid.clone());
            }

            stack.push((i, uuid.clone(), vec![]));
        }
    }

    while let Some((_, uuid, children)) = stack.pop() {
        if let Some(node) = nodes.get_mut(&uuid) {
            node.heading_uuids = children;
            node.headings_count = node.heading_uuids.len();
        }
    }
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

            // Collect outgoing links from headings without UUIDs into the primary note
            let heading_orphan_links: Vec<Link> = parsed
                .headings
                .iter()
                .filter(|h| h.uuid.is_none())
                .flat_map(|h| h.outgoing.clone())
                .collect();

            let mut node =
                Node::from_parsed(primary_uuid.clone(), title.clone(), path.clone(), &parsed);
            node.outgoing.extend(heading_orphan_links.clone());

            // Insert primary UUID into nodes
            nodes.insert(primary_uuid.clone(), node.clone());
            let mut primary_outgoing = parsed.outgoing.clone();
            primary_outgoing.extend(heading_orphan_links);
            uuid_to_outgoing.insert(primary_uuid.clone(), primary_outgoing);

            // Check heading UUIDs for uniqueness
            let mut file_heading_uuids_seen = std::collections::HashSet::new();
            let heading_uuids_list: Vec<String> = parsed.heading_uuids();
            for heading_uuid in &heading_uuids_list {
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

            // Process headings to create heading nodes with parent-child edges
            process_headings(
                &parsed.headings,
                primary_uuid,
                &parsed.filetags,
                &parsed.categories,
                &parsed.roam_aliases,
                &parsed.roam_refs,
                &path,
                &mut nodes,
                &mut uuid_to_outgoing,
                &mut heading_uuid_to_primary,
            );

            path_to_uuid.insert(path, primary_uuid.clone());
            title_to_uuid
                .entry(title)
                .or_default()
                .push(primary_uuid.clone());
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
            results: vec![],
        }
    }
}
