use crate::{Corpus, Graph, OrgConfig};
use anyhow::Result;

pub struct Workspace {
    pub corpus: Corpus,
    pub graph: Graph,
}

impl Workspace {
    pub fn load(config: &OrgConfig) -> Result<Self> {
        tracing::debug!(db_root = %config.db_root.display(), "loading workspace");
        let corpus = Corpus::load(config)?;
        let graph = Graph::from_corpus_without_raw_content(&corpus);
        tracing::debug!(
            file_count = corpus.results().len(),
            node_count = graph.nodes.len(),
            "workspace loaded"
        );
        Ok(Workspace { corpus, graph })
    }
}
