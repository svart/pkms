use crate::{Corpus, Graph, LinkResolutionContext, ScanConfig};
use anyhow::Result;

/// One fresh filesystem scan together with its parsed content and graph indexes.
pub struct OrgSnapshot {
    corpus: Corpus,
    graph: Graph,
}

impl OrgSnapshot {
    pub fn load(scan: &ScanConfig, links: &LinkResolutionContext) -> Result<Self> {
        tracing::debug!(db_root = %scan.db_root.display(), "loading org snapshot");
        let corpus = Corpus::load_from(scan)?;
        let mut graph = Graph::from_corpus_without_raw_content(&corpus);
        graph.apply_link_context(links);
        tracing::debug!(
            file_count = corpus.results().len(),
            node_count = graph.nodes().count(),
            "org snapshot loaded"
        );
        Ok(Self { corpus, graph })
    }

    pub fn corpus(&self) -> &Corpus {
        &self.corpus
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn into_parts(self) -> (Corpus, Graph) {
        (self.corpus, self.graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_parsed_content_and_graph_from_one_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.org");
        std::fs::write(
            &path,
            ":PROPERTIES:\n:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\n:END:\n#+title: Snapshot note\n\nBody\n",
        )
        .unwrap();
        let scan = ScanConfig {
            db_root: dir.path().to_path_buf(),
            ignore_patterns: Vec::new(),
            todo_states: vec!["TODO".to_string(), "DONE".to_string()],
        };
        let links = LinkResolutionContext {
            db_root: dir.path().to_path_buf(),
            home_dir: Some(PathBuf::from("/home/test")),
        };

        let snapshot = OrgSnapshot::load(&scan, &links).unwrap();

        assert_eq!(snapshot.corpus().results().len(), 1);
        assert_eq!(
            snapshot.corpus().results()[0].raw_content.as_deref(),
            Some(
                ":PROPERTIES:\n:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\n:END:\n#+title: Snapshot note\n\nBody\n"
            )
        );
        assert_eq!(
            snapshot
                .graph()
                .node("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
                .map(|node| node.title.as_str()),
            Some("Snapshot note")
        );
        assert_eq!(
            snapshot.graph().home_dir(),
            Some(PathBuf::from("/home/test").as_path())
        );
    }
}
