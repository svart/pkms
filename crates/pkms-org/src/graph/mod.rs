//! In-memory knowledge graph of org-roam notes.
//!
//! [`Graph`] is the central data structure, built from parsed `.org` files.
//! Each file is parsed into a [`ParsedNote`](crate::parser::ParsedNote)
//! (UUIDs, title, tags, aliases, links, headings). The builder promotes the primary UUID of
//! each file into a [`Node`] and creates separate heading-nodes for headings with their own
//! `:ID:` property. The graph resolves internal links into backlinks and detects broken links.

pub mod analytics;
pub mod builder;
pub mod search;
pub mod tasks;
pub mod traversal;
pub mod validation;

use crate::OrgConfig;
use crate::corpus::Corpus;
pub use crate::corpus::FileScanResult;
use crate::domain::{LinkTarget, NoteId};
use crate::parser::{Link, ParsedNote};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub uuid: NoteId,
    pub title: String,
    pub path: PathBuf,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub outgoing: Vec<Link>,
    pub headings_count: usize,
    pub heading_uuids: Vec<NoteId>,
    pub has_todos: bool,
}

impl Node {
    pub fn from_parsed(uuid: NoteId, title: String, path: PathBuf, parsed: &ParsedNote) -> Self {
        Node {
            uuid,
            title,
            path,
            filetags: parsed.filetags.clone(),
            categories: parsed.categories.clone(),
            aliases: parsed.aliases.clone(),
            refs: parsed.roam_refs.clone(),
            outgoing: parsed.outgoing.clone(),
            headings_count: parsed.headings.len(),
            heading_uuids: parsed.heading_uuids(),
            has_todos: parsed.has_todo_headings(),
        }
    }
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
    pub nodes: HashMap<NoteId, Node>,
    pub path_to_uuid: HashMap<PathBuf, NoteId>,
    pub title_to_uuid: HashMap<String, Vec<NoteId>>,
    pub alias_to_uuid: HashMap<String, Vec<NoteId>>,
    pub backlinks: HashMap<NoteId, Vec<NoteId>>,
    pub broken_links: Vec<(NoteId, NoteId)>,
    pub parse_errors: Vec<(PathBuf, String)>,
    pub skipped_files: Vec<PathBuf>,
    pub duplicates: DuplicateInfo,
    pub heading_uuid_to_primary: HashMap<NoteId, NoteId>,
    pub results: Vec<FileScanResult>,
    home_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingLocation {
    pub primary_uuid: NoteId,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfLinkEntry {
    pub source_uuid: NoteId,
    pub source_title: String,
    pub link_type: validation::SelfLinkKind,
    pub target: LinkTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverlinkEntry {
    pub source_uuid: NoteId,
    pub source_title: String,
    pub target_uuid: NoteId,
    pub target_title: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileLinkTarget {
    pub(crate) path: PathBuf,
    pub(crate) line_spec: Option<String>,
    pub(crate) org_relative: bool,
}

impl FileLinkTarget {
    pub(crate) fn parse(target_path: &str, home_dir: Option<&Path>) -> Self {
        let (inner_path, org_relative) = if let Some(rest) = target_path.strip_prefix("org:") {
            (rest, true)
        } else {
            (target_path, false)
        };
        let expanded = expand_file_link_home(inner_path, home_dir);
        let clean_path = expanded.split("::").next().unwrap_or(&expanded);
        let line_spec = target_path.split("::").nth(1).map(str::to_string);

        Self {
            path: PathBuf::from(clean_path),
            line_spec,
            org_relative,
        }
    }

    pub(crate) fn resolve_path(&self, source_path: &Path, db_root: &Path) -> PathBuf {
        if self.path.is_absolute() {
            self.path.clone()
        } else if self.org_relative {
            db_root.join(&self.path)
        } else {
            source_path.parent().unwrap_or(db_root).join(&self.path)
        }
    }
}

fn expand_file_link_home(inner_path: &str, home_dir: Option<&Path>) -> String {
    if inner_path.starts_with('~') {
        if let Some(home) = home_dir {
            inner_path.replacen('~', &home.display().to_string(), 1)
        } else {
            inner_path.to_string()
        }
    } else {
        inner_path.to_string()
    }
}

pub fn resolve_file_link_path(target_path: &str, source_path: &Path, db_root: &Path) -> PathBuf {
    resolve_file_link_path_with_home(target_path, source_path, db_root, None)
}

pub fn resolve_file_link_path_with_home(
    target_path: &str,
    source_path: &Path,
    db_root: &Path,
    home_dir: Option<&Path>,
) -> PathBuf {
    FileLinkTarget::parse(target_path, home_dir).resolve_path(source_path, db_root)
}

/// Check whether a file link target exists on disk and optionally matches a line spec.
pub fn file_link_target_exists(target: &str, source_path: &Path, db_root: &Path) -> bool {
    file_link_target_exists_with_home(target, source_path, db_root, None)
}

/// Check whether a file link target exists using an explicit home directory for `~` expansion.
pub fn file_link_target_exists_with_home(
    target: &str,
    source_path: &Path,
    db_root: &Path,
    home_dir: Option<&Path>,
) -> bool {
    let target = FileLinkTarget::parse(target, home_dir);
    let resolved = target.resolve_path(source_path, db_root);
    if !resolved.exists() {
        return false;
    }
    if let Some(line_spec) = target.line_spec.as_deref() {
        if line_spec.is_empty() {
            return true;
        }
        file_link_line_spec_exists(&resolved, line_spec)
    } else {
        true
    }
}

fn file_link_line_spec_exists(path: &Path, line_spec: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    if let Ok(line_number) = line_spec.parse::<usize>() {
        return line_number > 0 && content.lines().nth(line_number - 1).is_some();
    }
    content.lines().any(|line| line.contains(line_spec))
}

impl Graph {
    pub fn load(config: &OrgConfig) -> anyhow::Result<Self> {
        tracing::debug!(db_root = %config.db_root.display(), "loading graph");
        let corpus = Corpus::load(config)?;
        let mut graph = Self::from_corpus(&corpus);
        graph.home_dir = config.home_dir.clone();
        graph.index_db_relative_paths(&config.db_root);
        tracing::debug!(
            node_count = graph.nodes.len(),
            backlink_target_count = graph.backlinks.len(),
            broken_link_count = graph.broken_links.len(),
            parse_error_count = graph.parse_errors.len(),
            skipped_file_count = graph.skipped_files.len(),
            "graph loaded"
        );
        Ok(graph)
    }

    pub fn from_corpus(corpus: &Corpus) -> Self {
        Self::from_results(corpus.results().to_vec())
    }

    pub fn from_corpus_without_raw_content(corpus: &Corpus) -> Self {
        Self::from_results(
            corpus
                .results()
                .iter()
                .map(FileScanResult::without_raw_content)
                .collect(),
        )
    }

    fn from_results(results: Vec<FileScanResult>) -> Self {
        let mut graph = Graph::build_from_results(&results);
        graph.results = results;
        graph.home_dir = None;
        tracing::debug!(
            node_count = graph.nodes.len(),
            file_count = graph.results.len(),
            parse_error_count = graph.parse_errors.len(),
            skipped_file_count = graph.skipped_files.len(),
            "graph built from corpus"
        );
        graph
    }

    fn index_db_relative_paths(&mut self, db_root: &Path) {
        for result in &self.results {
            let Some(uuid) = self.path_to_uuid.get(&result.path).cloned() else {
                continue;
            };
            if let Ok(relative_path) = result.path.strip_prefix(db_root) {
                self.path_to_uuid
                    .entry(relative_path.to_path_buf())
                    .or_insert(uuid);
            }
        }
    }

    pub fn find_node(&self, target: &str) -> Option<&Node> {
        let target_id = NoteId::new(target);
        if let Some(node) = self.nodes.get(&target_id) {
            return Some(node);
        }
        if let Some(uuid) = self.path_to_uuid.get(&PathBuf::from(target)) {
            return self.nodes.get(uuid);
        }
        if let Ok(path) = std::fs::canonicalize(target)
            && let Some(uuid) = self.path_to_uuid.get(&path)
        {
            return self.nodes.get(uuid);
        }
        if let Some(uuids) = self.title_to_uuid.get(target)
            && let Some(uuid) = uuids.first()
        {
            return self.nodes.get(uuid);
        }
        if let Some(uuids) = self.alias_to_uuid.get(target)
            && let Some(uuid) = uuids.first()
        {
            return self.nodes.get(uuid);
        }
        if let Some(primary) = self.heading_uuid_to_primary.get(&target_id) {
            return self.nodes.get(primary);
        }
        None
    }

    pub fn resolve_target(&self, target: &str) -> anyhow::Result<&Node> {
        self.find_node(target)
            .ok_or_else(|| anyhow::anyhow!("Note not found: {target}"))
    }

    pub fn primary_uuid_for_heading(&self, heading_uuid: &str) -> Option<&str> {
        let heading_uuid = NoteId::new(heading_uuid);
        self.heading_uuid_to_primary
            .get(&heading_uuid)
            .map(NoteId::as_str)
    }

    pub fn heading_location(&self, heading_uuid: &str) -> Option<HeadingLocation> {
        let primary_uuid = self.primary_uuid_for_heading(heading_uuid)?;
        let primary = self.nodes.get(primary_uuid)?;
        let heading = self
            .results
            .iter()
            .find(|result| result.path == primary.path)?
            .parsed
            .headings
            .iter()
            .find(|heading| {
                heading
                    .uuid
                    .as_ref()
                    .is_some_and(|uuid| uuid.as_str() == heading_uuid)
            })?;

        Some(HeadingLocation {
            primary_uuid: NoteId::new(primary_uuid),
            line_number: heading.line_number,
        })
    }

    pub fn raw_content_for_path(&self, path: &Path) -> Option<&str> {
        self.results
            .iter()
            .find(|result| result.path == path)
            .and_then(|result| result.raw_content.as_deref())
    }

    pub fn home_dir(&self) -> Option<&Path> {
        self.home_dir.as_deref()
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
    pub total_ssh_links: usize,
    pub orphan_notes: usize,
    pub broken_link_count: usize,
    pub skipped_count: usize,
    pub parse_error_count: usize,
    pub duplicate_uuid_count: usize,
    pub duplicate_title_count: usize,
    pub missing_title_count: usize,
}

#[cfg(test)]
mod tests;
