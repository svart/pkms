use crate::embeddings::EmbeddingProvider;
use crate::models::{
    IngestSummary, RetrievalRecord, RetrieveRequest, RetrieveResponse, SearchResult, StatusResponse,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RagIndex {
    db_path: PathBuf,
}

impl RagIndex {
    pub fn open(db_path: impl Into<PathBuf>) -> Result<Self> {
        let index = Self {
            db_path: db_path.into(),
        };
        crate::db::connect(&index.db_path)?;
        Ok(index)
    }

    pub fn remove_files(db_path: impl AsRef<Path>) -> Result<()> {
        crate::db::remove_index_files(db_path)
    }

    pub fn status(&self) -> Result<StatusResponse> {
        let connection = crate::db::connect(&self.db_path)?;
        crate::db::status(&connection, &self.db_path)
    }

    pub fn ingest(
        &self,
        records: &[RetrievalRecord],
        embedding_provider: &dyn EmbeddingProvider,
        full_rebuild: bool,
    ) -> Result<IngestSummary> {
        let mut connection = crate::db::connect(&self.db_path)?;
        crate::db::ingest_records(&mut connection, records, embedding_provider, full_rebuild)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let connection = crate::db::connect(&self.db_path)?;
        crate::db::search(&connection, query, limit)
    }

    pub fn retrieve(
        &self,
        request: &RetrieveRequest,
        embedding_provider: &dyn EmbeddingProvider,
    ) -> Result<RetrieveResponse> {
        let connection = crate::db::connect(&self.db_path)?;
        crate::retrieve::retrieve(&connection, request, embedding_provider)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }
}
