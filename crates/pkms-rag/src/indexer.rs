use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::{
    db::{connect, ingest_records},
    embeddings::{
        EmbeddingProvider, EmbeddingProviderConfig, provider_from_config, provider_from_env,
    },
    models::{IndexProgress, IngestSummary, RetrievalRecord},
    ndjson::load_ndjson,
    org_export::export_org_notes,
};

#[derive(Debug, Clone)]
pub struct BackgroundIndexer {
    db_path: PathBuf,
    source_path: Option<PathBuf>,
    notes_root: Option<PathBuf>,
    progress: Arc<Mutex<IndexProgress>>,
    running: Arc<Mutex<bool>>,
}

impl BackgroundIndexer {
    pub fn new(
        db_path: impl Into<PathBuf>,
        source_path: Option<PathBuf>,
        notes_root: Option<PathBuf>,
    ) -> Self {
        let progress = IndexProgress {
            source_path: source_path.as_deref().map(display_path),
            notes_root: notes_root.as_deref().map(display_path),
            ..IndexProgress::default()
        };
        Self {
            db_path: db_path.into(),
            source_path,
            notes_root,
            progress: Arc::new(Mutex::new(progress)),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start(&self) -> IndexProgress {
        let (progress, _handle) = self.start_with_provider_factory(provider_from_env);
        progress
    }

    pub fn start_with_provider_config(&self, config: EmbeddingProviderConfig) -> IndexProgress {
        let (progress, _handle) =
            self.start_with_provider_factory(move || provider_from_config(&config));
        progress
    }

    pub fn run_sync_with_provider(&self, provider: &dyn EmbeddingProvider) -> IndexProgress {
        if !self.has_index_source() {
            self.set_no_source();
            return self.status();
        }

        let started_at = unix_timestamp_seconds();
        if !self.begin_rebuild(started_at) {
            return self.status();
        }
        self.run_rebuild_with_provider(started_at, provider);
        self.finish_rebuild();
        self.status()
    }

    pub fn run_sync_with_provider_config(&self, config: &EmbeddingProviderConfig) -> IndexProgress {
        if !self.has_index_source() {
            self.set_no_source();
            return self.status();
        }

        let started_at = unix_timestamp_seconds();
        if !self.begin_rebuild(started_at) {
            return self.status();
        }
        match provider_from_config(config) {
            Ok(provider) => self.run_rebuild_with_provider(started_at, provider.as_ref()),
            Err(err) => self.set_error(err.to_string()),
        }
        self.finish_rebuild();
        self.status()
    }

    pub fn status(&self) -> IndexProgress {
        self.progress().clone()
    }

    fn start_with_provider_factory<F>(
        &self,
        provider_factory: F,
    ) -> (IndexProgress, Option<JoinHandle<()>>)
    where
        F: FnOnce() -> Result<Box<dyn EmbeddingProvider>> + Send + 'static,
    {
        if !self.has_index_source() {
            self.set_no_source();
            return (self.status(), None);
        }

        let started_at = unix_timestamp_seconds();
        if !self.begin_rebuild(started_at) {
            return (self.status(), None);
        }

        let indexer = self.clone();
        match thread::Builder::new()
            .name("pkms-rag-indexer".to_string())
            .spawn(move || {
                indexer.run_rebuild_with_provider_factory(started_at, provider_factory);
                indexer.finish_rebuild();
            }) {
            Ok(handle) => (self.status(), Some(handle)),
            Err(err) => {
                self.set_error(format!("failed to spawn RAG indexer thread: {err}"));
                self.finish_rebuild();
                (self.status(), None)
            }
        }
    }

    fn run_rebuild_with_provider_factory<F>(&self, started_at: f64, provider_factory: F)
    where
        F: FnOnce() -> Result<Box<dyn EmbeddingProvider>>,
    {
        let result = (|| {
            let records = self.load_records(started_at)?;
            self.set(
                "indexing",
                "init-embedding-model",
                "Preparing embedding model.",
                Some(records.len() as u64),
            );
            let provider =
                provider_factory().context("failed to initialize RAG embedding provider")?;
            self.ingest_records(&records, provider.as_ref())
        })();
        if let Err(err) = result {
            self.set_error(err.to_string());
        }
    }

    fn run_rebuild_with_provider(&self, started_at: f64, provider: &dyn EmbeddingProvider) {
        let result = (|| {
            let records = self.load_records(started_at)?;
            self.set(
                "indexing",
                "init-embedding-model",
                "Preparing embedding model.",
                Some(records.len() as u64),
            );
            self.ingest_records(&records, provider)
        })();
        if let Err(err) = result {
            self.set_error(err.to_string());
        }
    }

    fn load_records(&self, started_at: f64) -> Result<Vec<RetrievalRecord>> {
        if let Some(notes_root) = &self.notes_root {
            self.set_loading(
                "export-notes-root",
                format!("Exporting org notes from {}", display_path(notes_root)),
                started_at,
            );
            return export_org_notes(notes_root);
        }

        let Some(source_path) = &self.source_path else {
            bail!("No notes root or index source configured.");
        };
        if !source_path.exists() {
            bail!("index source does not exist: {}", display_path(source_path));
        }
        self.set_loading(
            "read-ndjson",
            format!("Reading {}", display_path(source_path)),
            started_at,
        );
        load_ndjson(source_path)
    }

    fn ingest_records(
        &self,
        records: &[RetrievalRecord],
        provider: &dyn EmbeddingProvider,
    ) -> Result<()> {
        self.set(
            "indexing",
            "ingest-rebuild",
            format!("Rebuilding index from {} records.", records.len()),
            None,
        );
        let mut conn = connect(&self.db_path)?;
        let summary = ingest_records(&mut conn, records, provider, true)?;
        self.add_summary(&summary, records.len() as u64);
        self.set_complete(records.len() as u64);
        Ok(())
    }

    fn has_index_source(&self) -> bool {
        self.notes_root.is_some() || self.source_path.is_some()
    }

    fn begin_rebuild(&self, started_at: f64) -> bool {
        {
            let mut running = self.running();
            if *running {
                return false;
            }
            *running = true;
        }
        *self.progress() = IndexProgress {
            phase: "loading".to_string(),
            current_step: "starting".to_string(),
            message: "Starting index rebuild.".to_string(),
            source_path: self.source_path.as_deref().map(display_path),
            notes_root: self.notes_root.as_deref().map(display_path),
            started_at: Some(started_at),
            ..IndexProgress::default()
        };
        true
    }

    fn finish_rebuild(&self) {
        *self.running() = false;
    }

    fn set_no_source(&self) {
        let mut progress = self.progress();
        progress.phase = "idle".to_string();
        progress.current_step = "no-source".to_string();
        progress.message = "No notes root or index source configured.".to_string();
        progress.error = None;
    }

    fn set_loading(&self, current_step: &str, message: String, started_at: f64) {
        let mut progress = self.progress();
        progress.phase = "loading".to_string();
        progress.current_step = current_step.to_string();
        progress.message = message;
        progress.started_at = Some(started_at);
        progress.finished_at = None;
        progress.error = None;
    }

    fn set(&self, phase: &str, current_step: &str, message: impl Into<String>, total: Option<u64>) {
        let mut progress = self.progress();
        progress.phase = phase.to_string();
        progress.current_step = current_step.to_string();
        progress.message = message.into();
        if let Some(total) = total {
            progress.total_records = total;
        }
    }

    fn set_complete(&self, processed: u64) {
        let mut progress = self.progress();
        progress.phase = "complete".to_string();
        progress.current_step = "complete".to_string();
        progress.message = "Index is ready.".to_string();
        progress.processed_records = processed;
        progress.finished_at = Some(unix_timestamp_seconds());
        progress.error = None;
    }

    fn set_error(&self, error: String) {
        let mut progress = self.progress();
        progress.phase = "error".to_string();
        progress.current_step = "error".to_string();
        progress.message = error.clone();
        progress.error = Some(error);
        progress.finished_at = Some(unix_timestamp_seconds());
    }

    fn add_summary(&self, summary: &IngestSummary, processed_records: u64) {
        let mut progress = self.progress();
        progress.processed_records = processed_records;
        progress.notes_seen += summary.notes_seen;
        progress.notes_upserted += summary.notes_upserted;
        progress.chunks_seen += summary.chunks_seen;
        progress.chunks_upserted += summary.chunks_upserted;
        progress.chunks_unchanged += summary.chunks_unchanged;
        progress.links_seen += summary.links_seen;
        progress.links_upserted += summary.links_upserted;
        progress.deletes_seen += summary.deletes_seen;
        progress.notes_deleted += summary.notes_deleted;
        progress.chunks_deleted += summary.chunks_deleted;
        progress.embeddings_computed += summary.embeddings_computed;
        progress.embeddings_skipped += summary.embeddings_skipped;
    }

    fn progress(&self) -> MutexGuard<'_, IndexProgress> {
        self.progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn running(&self) -> MutexGuard<'_, bool> {
        self.running
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn unix_timestamp_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::status, embeddings::HashEmbeddingProvider};
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Condvar,
            mpsc::{self, Sender},
        },
        time::Duration,
    };

    #[test]
    fn indexer_reports_idle_when_no_source_is_configured() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let indexer = BackgroundIndexer::new(tempdir.path().join("rag.sqlite3"), None, None);
        let provider = HashEmbeddingProvider::default();

        let progress = indexer.run_sync_with_provider(&provider);

        assert_eq!(progress.phase, "idle");
        assert_eq!(progress.current_step, "no-source");
        assert!(progress.message.contains("No notes root"));
    }

    #[test]
    fn indexer_reports_error_for_missing_ndjson_source() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let source = tempdir.path().join("missing.ndjson");
        let indexer =
            BackgroundIndexer::new(tempdir.path().join("rag.sqlite3"), Some(source), None);
        let provider = HashEmbeddingProvider::default();

        let progress = indexer.run_sync_with_provider(&provider);

        assert_eq!(progress.phase, "error");
        assert_eq!(progress.current_step, "error");
        assert!(
            progress
                .error
                .expect("error is recorded")
                .contains("does not exist")
        );
    }

    #[test]
    fn indexer_rebuilds_from_ndjson_source() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("rag.sqlite3");
        let source = fixture_path();
        let indexer = BackgroundIndexer::new(db_path.clone(), Some(source.clone()), None);
        let provider = HashEmbeddingProvider::default();

        let progress = indexer.run_sync_with_provider(&provider);
        let conn = connect(&db_path).expect("database opens");
        let current = status(&conn, &db_path).expect("status reads");

        assert_eq!(progress.phase, "complete");
        assert_eq!(progress.current_step, "complete");
        assert_eq!(progress.source_path, Some(display_path(&source)));
        assert_eq!(progress.total_records, 5);
        assert_eq!(progress.processed_records, 5);
        assert_eq!(progress.notes_seen, 2);
        assert_eq!(progress.chunks_seen, 2);
        assert_eq!(progress.links_seen, 1);
        assert_eq!(progress.embeddings_computed, 2);
        assert_eq!(current.notes, 2);
        assert_eq!(current.chunks, 2);
        assert_eq!(current.links, 1);
    }

    #[test]
    fn indexer_rebuilds_from_notes_root_and_removes_deleted_notes() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let notes_root = tempdir.path().join("notes");
        fs::create_dir(&notes_root).expect("notes root creates");
        let note_path = notes_root.join("agent-search.org");
        fs::write(
            &note_path,
            "\
#+title: Agent Search
:PROPERTIES:
:ID: dddddddd-dddd-4ddd-dddd-dddddddddddd
:END:
* API usage
Agents call /retrieve to semantically search PKMS notes.
",
        )
        .expect("note writes");
        let db_path = tempdir.path().join("rag.sqlite3");
        let indexer = BackgroundIndexer::new(db_path.clone(), None, Some(notes_root.clone()));
        let provider = HashEmbeddingProvider::default();

        let first = indexer.run_sync_with_provider(&provider);
        let conn = connect(&db_path).expect("database opens");
        let current = status(&conn, &db_path).expect("status reads");
        assert_eq!(first.phase, "complete");
        assert_eq!(first.notes_root, Some(display_path(&notes_root)));
        assert_eq!(current.notes, 1);
        assert_eq!(current.chunks, 1);

        fs::remove_file(note_path).expect("note removes");
        let second = indexer.run_sync_with_provider(&provider);
        let current = status(&conn, &db_path).expect("status reads");

        assert_eq!(second.phase, "complete");
        assert_eq!(second.total_records, 0);
        assert_eq!(second.notes_deleted, 1);
        assert_eq!(second.chunks_deleted, 1);
        assert_eq!(current.notes, 0);
        assert_eq!(current.chunks, 0);
    }

    #[test]
    fn indexer_start_prevents_duplicate_background_rebuilds() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("rag.sqlite3");
        let source = fixture_path();
        let indexer = BackgroundIndexer::new(db_path, Some(source), None);
        let (entered_tx, entered_rx) = mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let provider_release = Arc::clone(&release);

        let (first, handle) = indexer.start_with_provider_factory(move || {
            Ok(Box::new(BlockingProvider {
                inner: HashEmbeddingProvider::default(),
                entered: entered_tx,
                release: provider_release,
            }))
        });
        assert_eq!(first.phase, "loading");
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("background provider starts");

        let (second, duplicate_handle) = indexer
            .start_with_provider_factory(|| panic!("duplicate start should not build a provider"));
        assert!(duplicate_handle.is_none());
        assert_ne!(second.phase, "complete");

        let (lock, cvar) = &*release;
        *lock.lock().expect("release lock") = true;
        cvar.notify_one();
        handle
            .expect("first start returns a join handle")
            .join()
            .expect("background thread joins");

        assert_eq!(indexer.status().phase, "complete");
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retrieval-export.ndjson")
    }

    struct BlockingProvider {
        inner: HashEmbeddingProvider,
        entered: Sender<()>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl EmbeddingProvider for BlockingProvider {
        fn model_name(&self) -> &str {
            self.inner.model_name()
        }

        fn dimension(&self) -> usize {
            self.inner.dimension()
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            self.entered.send(()).expect("test receiver is alive");
            let (lock, cvar) = &*self.release;
            let mut released = lock.lock().expect("release lock");
            while !*released {
                released = cvar.wait(released).expect("release lock waits");
            }
            self.inner.embed(texts)
        }
    }
}
