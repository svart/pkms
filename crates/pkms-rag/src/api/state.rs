use super::NoteViewer;
use anyhow::{Context, Result};
use std::{path::PathBuf, sync::Arc};

use crate::{
    BackgroundIndexer, EmbeddingProviderConfig,
    embeddings::{embedding_provider_config_from_env, provider_from_config},
    models::IndexProgress,
};

pub const DEFAULT_RAG_DB: &str = ".data/pkms-rag.sqlite3";

const RAG_DB_ENV: &str = "PKMS_RAG_DB";
const RAG_INDEX_SOURCE_ENV: &str = "PKMS_RAG_INDEX_SOURCE";
const RAG_NOTES_ROOT_ENV: &str = "PKMS_RAG_NOTES_ROOT";

#[derive(Clone)]
pub struct AppState {
    pub(super) db_path: PathBuf,
    pub(super) indexer: BackgroundIndexer,
    embedding_provider_config: EmbeddingProviderConfig,
    synchronous_indexing: bool,
    pub(super) note_viewer: Option<Arc<dyn NoteViewer>>,
}

impl AppState {
    pub fn from_env() -> Result<Self> {
        let db_path = std::env::var(RAG_DB_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_RAG_DB));
        let index_source = env_path(RAG_INDEX_SOURCE_ENV);
        let notes_root = env_path(RAG_NOTES_ROOT_ENV);
        let embedding_provider_config = embedding_provider_config_from_env()
            .context("failed to read RAG embedding provider configuration")?;
        Ok(Self::with_embedding_provider_config(
            db_path,
            index_source,
            notes_root,
            embedding_provider_config,
        ))
    }

    pub fn with_embedding_provider_config(
        db_path: impl Into<PathBuf>,
        index_source: Option<PathBuf>,
        notes_root: Option<PathBuf>,
        embedding_provider_config: EmbeddingProviderConfig,
    ) -> Self {
        let db_path = db_path.into();
        let indexer = BackgroundIndexer::new(db_path.clone(), index_source, notes_root);
        Self {
            db_path,
            indexer,
            embedding_provider_config,
            synchronous_indexing: false,
            note_viewer: None,
        }
    }

    pub fn with_synchronous_indexing(mut self, synchronous_indexing: bool) -> Self {
        self.synchronous_indexing = synchronous_indexing;
        self
    }

    pub fn with_note_viewer(mut self, note_viewer: Arc<dyn NoteViewer>) -> Self {
        self.note_viewer = Some(note_viewer);
        self
    }

    pub(super) fn provider(&self) -> Result<Box<dyn crate::EmbeddingProvider>> {
        provider_from_config(&self.embedding_provider_config)
    }

    pub(super) fn start_indexing(&self) -> IndexProgress {
        if self.synchronous_indexing {
            self.indexer
                .run_sync_with_provider_config(&self.embedding_provider_config)
        } else {
            self.indexer
                .start_with_provider_config(self.embedding_provider_config.clone())
        }
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}
