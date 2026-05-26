use crate::config::ResolvedConfig;
use crate::corpus::Corpus;
use crate::graph::Graph;
use anyhow::Result;

pub struct Workspace {
    pub corpus: Corpus,
    pub graph: Graph,
}

impl Workspace {
    pub fn load(config: &ResolvedConfig) -> Result<Self> {
        tracing::debug!(db_root = %config.resolved_db_root().display(), "loading workspace");
        let corpus = Corpus::load(config)?;
        let graph = Graph::from_corpus(&corpus);
        tracing::debug!(
            file_count = corpus.results().len(),
            node_count = graph.nodes.len(),
            "workspace loaded"
        );
        Ok(Workspace { corpus, graph })
    }
}
