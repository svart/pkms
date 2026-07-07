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
        let embedding_provider_config = embedding_provider_config_from_env()
            .context("failed to read RAG embedding provider configuration")?;
        Ok(Self::with_embedding_provider_config(
            db_path,
            None,
            None,
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

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn from_env_ignores_removed_source_env_vars() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        set_env("PKMS_RAG_EMBEDDING_PROVIDER", "hash");
        set_env("PKMS_RAG_INDEX_SOURCE", "/env/source.ndjson");
        set_env("PKMS_RAG_NOTES_ROOT", "/env/notes");

        let state = AppState::from_env().expect("state builds");

        let progress = state.indexer.status();
        assert_eq!(progress.source_path, None);
        assert_eq!(progress.notes_root, None);
    }

    struct EnvSnapshot {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvSnapshot {
        fn capture() -> Self {
            Self {
                values: vec![
                    ("PKMS_RAG_DB", std::env::var("PKMS_RAG_DB").ok()),
                    (
                        "PKMS_RAG_EMBEDDING_PROVIDER",
                        std::env::var("PKMS_RAG_EMBEDDING_PROVIDER").ok(),
                    ),
                    (
                        "PKMS_RAG_INDEX_SOURCE",
                        std::env::var("PKMS_RAG_INDEX_SOURCE").ok(),
                    ),
                    (
                        "PKMS_RAG_NOTES_ROOT",
                        std::env::var("PKMS_RAG_NOTES_ROOT").ok(),
                    ),
                ],
            }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => set_env(key, value),
                    None => remove_env(key),
                }
            }
        }
    }

    fn set_env(key: &str, value: &str) {
        // SAFETY: these tests serialize environment changes with ENV_LOCK and restore values.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: these tests serialize environment changes with ENV_LOCK and restore values.
        unsafe { std::env::remove_var(key) };
    }
}
