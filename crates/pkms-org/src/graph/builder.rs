use std::collections::HashMap;
use std::path::Path;

use crate::domain::NoteId;
use crate::parser::{Link, ParsedNote};

use super::{DuplicateEntry, DuplicateInfo, FileScanResult, Graph, Node};

type Backlinks = HashMap<NoteId, Vec<NoteId>>;
type BrokenLinks = Vec<(NoteId, NoteId)>;

struct HeadingStackEntry {
    index: usize,
    uuid: NoteId,
    children: Vec<NoteId>,
}

fn build_links(
    uuid_to_outgoing: &HashMap<NoteId, Vec<Link>>,
    nodes: &HashMap<NoteId, Node>,
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

impl BuildContext {
    fn process_headings(&mut self, primary_uuid: &NoteId, parsed: &ParsedNote, path: &Path) {
        let headings = &parsed.headings;
        let filetags = &parsed.filetags;
        let categories = &parsed.categories;
        let aliases = &parsed.aliases;
        let refs = &parsed.roam_refs;

        let mut stack: Vec<HeadingStackEntry> = Vec::new();

        for (i, heading) in headings.iter().enumerate() {
            while let Some(entry) = stack.last() {
                if headings[entry.index].level >= heading.level {
                    let entry = stack.pop().unwrap();
                    let uuid = entry.uuid;
                    if let Some(node) = self.nodes.get_mut(&uuid) {
                        node.heading_uuids = entry.children;
                        node.headings_count = node.heading_uuids.len();
                    }
                } else {
                    break;
                }
            }

            if let Some(uuid) = &heading.uuid {
                let parent_uuid = stack
                    .last()
                    .map(|entry| entry.uuid.clone())
                    .unwrap_or_else(|| primary_uuid.clone());

                let mut heading_outgoing = heading.outgoing.clone();
                heading_outgoing.push(Link::Internal(parent_uuid.clone()));

                let heading_node = Node {
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
                    outgoing: heading_outgoing.clone(),
                    headings_count: 0,
                    heading_uuids: vec![],
                    has_todos: heading.todo_state.is_some(),
                };

                self.uuid_to_outgoing.insert(uuid.clone(), heading_outgoing);

                self.uuid_to_outgoing
                    .entry(parent_uuid.clone())
                    .or_default()
                    .push(Link::Internal(uuid.clone()));

                self.heading_uuid_to_primary
                    .insert(uuid.clone(), primary_uuid.clone());

                self.nodes.insert(uuid.clone(), heading_node);

                if let Some(entry) = stack.last_mut() {
                    entry.children.push(uuid.clone());
                }

                stack.push(HeadingStackEntry {
                    index: i,
                    uuid: uuid.clone(),
                    children: vec![],
                });
            }
        }

        while let Some(entry) = stack.pop() {
            if let Some(node) = self.nodes.get_mut(&entry.uuid) {
                node.heading_uuids = entry.children;
                node.headings_count = node.heading_uuids.len();
            }
        }
    }
}

struct BuildContext {
    nodes: HashMap<NoteId, Node>,
    path_to_uuid: HashMap<std::path::PathBuf, NoteId>,
    title_to_uuid: HashMap<String, Vec<NoteId>>,
    alias_to_uuid: HashMap<String, Vec<NoteId>>,
    parse_errors: Vec<(std::path::PathBuf, String)>,
    skipped_files: Vec<std::path::PathBuf>,
    missing_titles: Vec<std::path::PathBuf>,
    seen_uuids: HashMap<NoteId, std::path::PathBuf>,
    duplicate_uuids: Vec<DuplicateEntry>,
    seen_titles: HashMap<String, std::path::PathBuf>,
    duplicate_titles: Vec<DuplicateEntry>,
    uuid_to_outgoing: HashMap<NoteId, Vec<Link>>,
    heading_uuid_to_primary: HashMap<NoteId, NoteId>,
    all_uuids_seen: HashMap<NoteId, std::path::PathBuf>,
}

impl BuildContext {
    fn new() -> Self {
        BuildContext {
            nodes: HashMap::new(),
            path_to_uuid: HashMap::new(),
            title_to_uuid: HashMap::new(),
            alias_to_uuid: HashMap::new(),
            parse_errors: Vec::new(),
            skipped_files: Vec::new(),
            missing_titles: Vec::new(),
            seen_uuids: HashMap::new(),
            duplicate_uuids: Vec::new(),
            seen_titles: HashMap::new(),
            duplicate_titles: Vec::new(),
            uuid_to_outgoing: HashMap::new(),
            heading_uuid_to_primary: HashMap::new(),
            all_uuids_seen: HashMap::new(),
        }
    }

    fn push_duplicate_uuid(&mut self, uuid: &NoteId, first_path: &Path, second_path: &Path) {
        self.duplicate_uuids.push(DuplicateEntry {
            value: uuid.to_string(),
            paths: vec![
                first_path.display().to_string(),
                second_path.display().to_string(),
            ],
        });
    }

    fn process_result(&mut self, result: &FileScanResult) {
        if let Some(err) = &result.parse_error {
            self.parse_errors.push((result.path.clone(), err.clone()));
            self.skipped_files.push(result.path.clone());
            return;
        }

        let parsed = &result.parsed;
        let path = &result.path;

        if parsed.uuids.is_empty() {
            self.skipped_files.push(path.clone());
            return;
        }

        let primary_uuid = parsed.uuids[0].clone();

        if let Some(existing) = self.seen_uuids.get(primary_uuid.as_str()).cloned()
            && existing.as_path() != path.as_path()
        {
            self.push_duplicate_uuid(&primary_uuid, &existing, path);
            return;
        }

        if let Some(existing) = self.all_uuids_seen.get(primary_uuid.as_str()).cloned()
            && existing.as_path() != path.as_path()
        {
            self.push_duplicate_uuid(&primary_uuid, &existing, path);
        }

        self.seen_uuids.insert(primary_uuid.clone(), path.clone());
        self.all_uuids_seen
            .insert(primary_uuid.clone(), path.clone());

        let title = if let Some(t) = parsed.title.clone() {
            t
        } else {
            self.missing_titles.push(path.clone());
            path.file_stem()
                .map(|s| s.display().to_string())
                .unwrap_or_default()
        };

        if let Some(existing) = self.seen_titles.get(&title) {
            self.duplicate_titles.push(DuplicateEntry {
                value: title.clone(),
                paths: vec![existing.display().to_string(), path.display().to_string()],
            });
        } else {
            self.seen_titles.insert(title.clone(), path.clone());
        }

        for alias in &parsed.aliases {
            self.alias_to_uuid
                .entry(alias.clone())
                .or_default()
                .push(primary_uuid.clone());
        }

        let heading_orphan_links: Vec<Link> = parsed
            .headings
            .iter()
            .filter(|h| h.uuid.is_none())
            .flat_map(|h| h.outgoing.clone())
            .collect();

        let mut primary_outgoing = parsed.outgoing.clone();
        primary_outgoing.extend(heading_orphan_links);

        self.uuid_to_outgoing
            .insert(primary_uuid.clone(), primary_outgoing);

        let mut node = Node::from_parsed(primary_uuid.clone(), title.clone(), path.clone(), parsed);
        node.outgoing = self.uuid_to_outgoing[&primary_uuid].clone();

        self.nodes.insert(primary_uuid.clone(), node);

        self.check_heading_uuids(parsed, &primary_uuid, path);

        self.process_headings(&primary_uuid, parsed, path);

        self.path_to_uuid.insert(path.clone(), primary_uuid.clone());
        self.title_to_uuid
            .entry(title)
            .or_default()
            .push(primary_uuid.clone());
    }

    fn check_heading_uuids(
        &mut self,
        parsed: &ParsedNote,
        primary_uuid: &NoteId,
        path: &std::path::Path,
    ) {
        let mut file_heading_uuids_seen = std::collections::HashSet::new();
        let heading_uuids_list = parsed.heading_uuids();
        for heading_uuid in &heading_uuids_list {
            if heading_uuid == primary_uuid {
                self.push_duplicate_uuid(heading_uuid, path, path);
                continue;
            }
            if !file_heading_uuids_seen.insert(heading_uuid.clone()) {
                self.push_duplicate_uuid(heading_uuid, path, path);
                continue;
            }
            if let Some(existing) = self.all_uuids_seen.get(heading_uuid.as_str()).cloned()
                && existing != path
            {
                self.push_duplicate_uuid(heading_uuid, &existing, path);
            }
            self.all_uuids_seen
                .insert(heading_uuid.clone(), path.to_path_buf());
        }
    }

    fn finalize(self) -> Graph {
        let (backlinks, broken_links) = build_links(&self.uuid_to_outgoing, &self.nodes);

        Graph {
            nodes: self.nodes,
            path_to_uuid: self.path_to_uuid,
            title_to_uuid: self.title_to_uuid,
            alias_to_uuid: self.alias_to_uuid,
            backlinks,
            broken_links,
            parse_errors: self.parse_errors,
            skipped_files: self.skipped_files,
            duplicates: DuplicateInfo {
                duplicate_uuids: self.duplicate_uuids,
                duplicate_titles: self.duplicate_titles,
                missing_titles: self
                    .missing_titles
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
            },
            heading_uuid_to_primary: self.heading_uuid_to_primary,
            results: vec![],
            home_dir: None,
        }
    }
}

impl Graph {
    pub fn build(results: Vec<FileScanResult>) -> Self {
        let mut graph = Self::build_from_results(&results);
        graph.results = results;
        graph
    }

    pub(crate) fn build_from_results(results: &[FileScanResult]) -> Self {
        let mut ctx = BuildContext::new();
        for result in results {
            ctx.process_result(result);
        }
        ctx.finalize()
    }
}
