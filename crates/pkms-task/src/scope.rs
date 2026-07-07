use crate::model::TaskItem;
use pkms_org::Graph;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedScope {
    raw: Vec<String>,
    paths: HashSet<String>,
}

impl ResolvedScope {
    pub fn resolve(graph: &Graph, db_root: &Path, raw: &[String]) -> Self {
        let paths = resolve_paths(graph, db_root, raw)
            .into_iter()
            .map(|path| path.display().to_string())
            .collect();
        Self {
            raw: raw.to_vec(),
            paths,
        }
    }

    pub fn matches_path(&self, path: &str) -> bool {
        self.paths.contains(path) || self.raw.iter().any(|scope| scope == path)
    }

    pub fn matches_task_item(&self, item: &TaskItem) -> bool {
        let path = item.path.as_ref().map(|path| path.display().to_string());
        if path.as_deref().is_some_and(|path| self.matches_path(path)) {
            return true;
        }

        self.raw.iter().any(|scope| {
            item.note_uuid.as_deref() == Some(scope.as_str())
                || item.note_title.as_deref() == Some(scope.as_str())
                || item.source_id == *scope
                || item.display_id == *scope
        })
    }
}

fn resolve_paths(graph: &Graph, db_root: &Path, raw: &[String]) -> Vec<PathBuf> {
    let mut scope_paths = Vec::new();
    for target in raw {
        if let Some(node) = graph.find_node(target) {
            scope_paths.push(node.path.clone());
            continue;
        }

        let expanded = if let Some(rest) = target.strip_prefix("~/") {
            graph.home_dir().map(|home| home.join(rest))
        } else {
            None
        };
        let mut matched = false;
        for candidate in [Some(Path::new(target)), expanded.as_deref()]
            .into_iter()
            .flatten()
        {
            for path in [candidate.to_path_buf()]
                .into_iter()
                .chain(candidate.canonicalize().ok())
            {
                if graph.results.iter().any(|result| result.path == path) {
                    scope_paths.push(path);
                    matched = true;
                    break;
                }
            }
            if matched {
                break;
            }
        }
        if matched {
            continue;
        }

        let joined = db_root.join(target);
        for path in [joined.clone()]
            .into_iter()
            .chain(joined.canonicalize().ok())
        {
            if graph.results.iter().any(|result| result.path == path) {
                scope_paths.push(path);
                break;
            }
        }
    }
    scope_paths
}
