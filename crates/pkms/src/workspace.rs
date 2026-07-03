use crate::config::ResolvedConfig;
use crate::graph::Graph;
use anyhow::Result;
use pkms_org::Corpus;

pub struct Workspace {
    pub corpus: Corpus,
    pub graph: Graph,
}

impl Workspace {
    pub fn load(config: &ResolvedConfig) -> Result<Self> {
        tracing::debug!(db_root = %config.resolved_db_root().display(), "loading workspace");
        let corpus = Corpus::load(&config.org_config())?;
        let graph = Graph::from_corpus_without_raw_content(&corpus);
        tracing::debug!(
            file_count = corpus.results().len(),
            node_count = graph.nodes.len(),
            "workspace loaded"
        );
        Ok(Workspace { corpus, graph })
    }
}
