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
        let corpus = Corpus::load(config)?;
        let graph = Graph::from_corpus(&corpus);
        Ok(Workspace { corpus, graph })
    }
}
