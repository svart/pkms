use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::{
    db::{IngestProgress, connect, ingest_records_with_progress},
    embeddings::{
        DEFAULT_EMBEDDING_MAX_BODY_CHARS, EmbeddingProvider, EmbeddingProviderConfig,
        default_embedding_provider_config, provider_from_config,
    },
    models::{IndexProgress, IngestSummary, RetrievalRecord},
    ndjson::load_ndjson,
    org_export::export_org_notes,
};

type ProgressCallback<'a> = dyn Fn(&IndexProgress) + 'a;

#[derive(Debug, Clone)]
pub struct BackgroundIndexer {
    db_path: PathBuf,
    source_path: Option<PathBuf>,
    notes_root: Option<PathBuf>,
    embedding_max_body_chars: usize,
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
            embedding_max_body_chars: DEFAULT_EMBEDDING_MAX_BODY_CHARS,
            progress: Arc::new(Mutex::new(progress)),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_embedding_max_body_chars(mut self, max_body_chars: usize) -> Self {
        self.embedding_max_body_chars = max_body_chars;
        self
    }

    pub fn start(&self) -> IndexProgress {
        self.start_with_provider_config(default_embedding_provider_config())
    }

    pub fn start_with_provider_config(&self, config: EmbeddingProviderConfig) -> IndexProgress {
        let (progress, _handle) =
            self.start_with_provider_factory(move || provider_from_config(&config));
        progress
    }

    pub fn run_sync_with_provider(&self, provider: &dyn EmbeddingProvider) -> IndexProgress {
        self.run_sync_with_provider_internal(provider, None)
    }

    pub fn run_sync_with_provider_and_progress(
        &self,
        provider: &dyn EmbeddingProvider,
        on_progress: impl Fn(&IndexProgress),
    ) -> IndexProgress {
        self.run_sync_with_provider_internal(provider, Some(&on_progress))
    }

    fn run_sync_with_provider_internal(
        &self,
        provider: &dyn EmbeddingProvider,
        on_progress: Option<&ProgressCallback<'_>>,
    ) -> IndexProgress {
        if !self.has_index_source() {
            self.set_no_source();
            self.emit_progress(on_progress);
            return self.status();
        }

        let started_at = unix_timestamp_seconds();
        if !self.begin_rebuild(started_at) {
            self.emit_progress(on_progress);
            return self.status();
        }
        self.emit_progress(on_progress);
        self.run_rebuild_with_provider(started_at, provider, on_progress);
        self.finish_rebuild();
        self.status()
    }

    pub fn run_sync_with_provider_config(&self, config: &EmbeddingProviderConfig) -> IndexProgress {
        self.run_sync_with_provider_config_internal(config, None)
    }

    pub fn run_sync_with_provider_config_and_progress(
        &self,
        config: &EmbeddingProviderConfig,
        on_progress: impl Fn(&IndexProgress),
    ) -> IndexProgress {
        self.run_sync_with_provider_config_internal(config, Some(&on_progress))
    }

    fn run_sync_with_provider_config_internal(
        &self,
        config: &EmbeddingProviderConfig,
        on_progress: Option<&ProgressCallback<'_>>,
    ) -> IndexProgress {
        if !self.has_index_source() {
            self.set_no_source();
            self.emit_progress(on_progress);
            return self.status();
        }

        let started_at = unix_timestamp_seconds();
        if !self.begin_rebuild(started_at) {
            self.emit_progress(on_progress);
            return self.status();
        }
        self.emit_progress(on_progress);
        match provider_from_config(config) {
            Ok(provider) => {
                self.run_rebuild_with_provider(started_at, provider.as_ref(), on_progress)
            }
            Err(err) => {
                self.set_error(format_error_chain(
                    &err.context("failed to initialize RAG embedding provider"),
                ));
                self.emit_progress(on_progress);
            }
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
        self.run_rebuild_with_provider_factory_with_progress(started_at, provider_factory, None);
    }

    fn run_rebuild_with_provider_factory_with_progress<F>(
        &self,
        started_at: f64,
        provider_factory: F,
        on_progress: Option<&ProgressCallback<'_>>,
    ) where
        F: FnOnce() -> Result<Box<dyn EmbeddingProvider>>,
    {
        let result = (|| {
            let records = self.load_records(started_at, on_progress)?;
            self.set(
                "indexing",
                "init-embedding-model",
                "Preparing embedding model.",
                Some(records.len() as u64),
            );
            self.emit_progress(on_progress);
            let provider =
                provider_factory().context("failed to initialize RAG embedding provider")?;
            self.ingest_records(&records, provider.as_ref(), on_progress)
        })();
        if let Err(err) = result {
            self.set_error(format_error_chain(&err));
            self.emit_progress(on_progress);
        }
    }

    fn run_rebuild_with_provider(
        &self,
        started_at: f64,
        provider: &dyn EmbeddingProvider,
        on_progress: Option<&ProgressCallback<'_>>,
    ) {
        let result = (|| {
            let records = self.load_records(started_at, on_progress)?;
            self.set(
                "indexing",
                "init-embedding-model",
                "Preparing embedding model.",
                Some(records.len() as u64),
            );
            self.emit_progress(on_progress);
            self.ingest_records(&records, provider, on_progress)
        })();
        if let Err(err) = result {
            self.set_error(format_error_chain(&err));
            self.emit_progress(on_progress);
        }
    }

    fn load_records(
        &self,
        started_at: f64,
        on_progress: Option<&ProgressCallback<'_>>,
    ) -> Result<Vec<RetrievalRecord>> {
        if let Some(notes_root) = &self.notes_root {
            self.set_loading(
                "export-notes-root",
                format!("Exporting org notes from {}", display_path(notes_root)),
                started_at,
            );
            self.emit_progress(on_progress);
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
        self.emit_progress(on_progress);
        load_ndjson(source_path)
    }

    fn ingest_records(
        &self,
        records: &[RetrievalRecord],
        provider: &dyn EmbeddingProvider,
        on_progress: Option<&ProgressCallback<'_>>,
    ) -> Result<()> {
        self.set(
            "indexing",
            "ingest-rebuild",
            format!("Rebuilding index from {} records.", records.len()),
            None,
        );
        self.emit_progress(on_progress);
        let mut conn = connect(&self.db_path)?;
        let summary = ingest_records_with_progress(
            &mut conn,
            records,
            provider,
            true,
            self.embedding_max_body_chars,
            |progress| {
                self.set_ingest_progress(progress);
                self.emit_progress(on_progress);
            },
        )?;
        self.set_summary(&summary, records.len() as u64);
        self.set_complete(records.len() as u64);
        self.emit_progress(on_progress);
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

    fn set_ingest_progress(&self, ingest_progress: &IngestProgress) {
        let mut progress = self.progress();
        progress.phase = "indexing".to_string();
        progress.current_step = ingest_progress.current_step.clone();
        progress.message = ingest_progress.message.clone();
        progress.total_records = ingest_progress.total_records;
        progress.processed_records = ingest_progress.processed_records;
        progress.total_embeddings = ingest_progress.total_embeddings;
        progress.processed_embeddings = ingest_progress.processed_embeddings;
        apply_summary(&mut progress, &ingest_progress.summary);
    }

    fn set_summary(&self, summary: &IngestSummary, processed_records: u64) {
        let mut progress = self.progress();
        progress.processed_records = processed_records;
        apply_summary(&mut progress, summary);
    }

    fn emit_progress(&self, on_progress: Option<&ProgressCallback<'_>>) {
        if let Some(on_progress) = on_progress {
            on_progress(&self.status());
        }
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

fn format_error_chain(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

fn apply_summary(progress: &mut IndexProgress, summary: &IngestSummary) {
    progress.notes_seen = summary.notes_seen;
    progress.notes_upserted = summary.notes_upserted;
    progress.chunks_seen = summary.chunks_seen;
    progress.chunks_upserted = summary.chunks_upserted;
    progress.chunks_unchanged = summary.chunks_unchanged;
    progress.links_seen = summary.links_seen;
    progress.links_upserted = summary.links_upserted;
    progress.deletes_seen = summary.deletes_seen;
    progress.notes_deleted = summary.notes_deleted;
    progress.chunks_deleted = summary.chunks_deleted;
    progress.embeddings_computed = summary.embeddings_computed;
    progress.embeddings_skipped = summary.embeddings_skipped;
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
    fn indexer_reports_sync_rebuild_progress() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("rag.sqlite3");
        let source = fixture_path();
        let indexer = BackgroundIndexer::new(db_path, Some(source), None);
        let provider = HashEmbeddingProvider::default();
        let events = Mutex::new(Vec::new());

        let progress = indexer.run_sync_with_provider_and_progress(&provider, |progress| {
            events.lock().expect("events lock").push(progress.clone());
        });

        let events = events.lock().expect("events lock");
        assert_eq!(progress.phase, "complete");
        assert!(
            events.iter().any(|event| {
                event.current_step == "ingest-records"
                    && event.total_records == 5
                    && event.processed_records > 0
                    && event.processed_records < event.total_records
            }),
            "expected a partial record ingest progress event, got {events:#?}"
        );
        assert!(
            events.iter().any(|event| {
                event.current_step == "embed-chunks"
                    && event.total_embeddings == 2
                    && event.processed_embeddings > 0
            }),
            "expected an embedding progress event, got {events:#?}"
        );
        assert_eq!(
            events.last().expect("at least one progress event").phase,
            "complete"
        );
    }

    #[test]
    fn indexer_reports_provider_initialization_error_chain() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("rag.sqlite3");
        let source = fixture_path();
        let indexer = BackgroundIndexer::new(db_path, Some(source), None);

        let (_progress, handle) = indexer.start_with_provider_factory(|| {
            Err(anyhow::anyhow!("corporate CA rejected").context("download request failed"))
        });
        handle
            .expect("provider factory starts in the background")
            .join()
            .expect("background thread joins");

        let progress = indexer.status();
        let error = progress.error.expect("error is recorded");
        assert_eq!(progress.phase, "error");
        assert!(
            error.contains(
                "failed to initialize RAG embedding provider: download request failed: corporate CA rejected"
            ),
            "error should preserve the provider error chain: {error}"
        );
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
