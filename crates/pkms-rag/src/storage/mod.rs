//! Public RAG index facade over the private SQLite implementation.

use crate::embeddings::EmbeddingProvider;
use crate::models::{
    IngestSummary, RetrievalRecord, RetrieveRequest, RetrieveResponse, SearchResult,
    StatusResponse, TagRecommendation, TagRecommendationRequest,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(crate) mod sqlite;

#[derive(Debug, Clone)]
pub struct RagIndex {
    db_path: PathBuf,
}

impl RagIndex {
    pub fn open(db_path: impl Into<PathBuf>) -> Result<Self> {
        let index = Self {
            db_path: db_path.into(),
        };
        sqlite::connect(&index.db_path)?;
        Ok(index)
    }

    pub fn remove_files(db_path: impl AsRef<Path>) -> Result<()> {
        sqlite::remove_index_files(db_path)
    }

    pub fn status(&self) -> Result<StatusResponse> {
        let connection = sqlite::connect(&self.db_path)?;
        sqlite::status(&connection, &self.db_path)
    }

    pub fn ingest(
        &self,
        records: &[RetrievalRecord],
        embedding_provider: &dyn EmbeddingProvider,
        full_rebuild: bool,
    ) -> Result<IngestSummary> {
        let mut connection = sqlite::connect(&self.db_path)?;
        sqlite::ingest_records(&mut connection, records, embedding_provider, full_rebuild)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let connection = sqlite::connect(&self.db_path)?;
        sqlite::search(&connection, query, limit)
    }

    pub fn retrieve(
        &self,
        request: &RetrieveRequest,
        embedding_provider: &dyn EmbeddingProvider,
    ) -> Result<RetrieveResponse> {
        let connection = sqlite::connect(&self.db_path)?;
        crate::retrieve::retrieve(&connection, request, embedding_provider)
    }

    pub fn recommend_tags(
        &self,
        request: &TagRecommendationRequest,
        embedding_provider: &dyn EmbeddingProvider,
    ) -> Result<Vec<TagRecommendation>> {
        let connection = sqlite::connect(&self.db_path)?;
        let status = sqlite::status(&connection, &self.db_path)?;
        anyhow::ensure!(
            status.notes > 0 && status.embeddings > 0,
            "RAG index has no embedded notes; run `pkms rag index` first"
        );
        let compatible_embeddings = sqlite::compatible_embedding_count(
            &connection,
            embedding_provider.model_name(),
            embedding_provider.dimension(),
        )?;
        anyhow::ensure!(
            compatible_embeddings > 0,
            "RAG index does not contain compatible embeddings for model {} with dimension {}; run `pkms rag index` with the configured embedding provider",
            embedding_provider.model_name(),
            embedding_provider.dimension()
        );
        crate::tags::recommend_tags(&connection, request, embedding_provider)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }
}
