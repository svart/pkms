use super::NoteViewer;
use anyhow::Result;
use std::{path::PathBuf, sync::Arc};

use crate::{
    BackgroundIndexer, EmbeddingProviderConfig, embeddings::provider_from_config,
    models::IndexProgress,
};

pub const DEFAULT_RAG_DB: &str = ".data/pkms-rag.sqlite3";

#[derive(Clone)]
pub struct AppState {
    pub(super) db_path: PathBuf,
    pub(super) indexer: BackgroundIndexer,
    embedding_provider_config: EmbeddingProviderConfig,
    synchronous_indexing: bool,
    pub(super) note_viewer: Option<Arc<dyn NoteViewer>>,
}

impl AppState {
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
                .unwrap_or_else(|_| self.indexer.status())
        } else {
            self.indexer
                .start_with_provider_config(self.embedding_provider_config.clone())
        }
    }
}
