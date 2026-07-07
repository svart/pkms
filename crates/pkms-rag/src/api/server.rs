use super::{AppState, NoteViewer, router};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{EmbeddingProviderConfig, embeddings::default_embedding_provider_config};

#[derive(Debug, Clone)]
pub struct RagServeOptions {
    pub db_path: PathBuf,
    pub index_source: Option<PathBuf>,
    pub notes_root: Option<PathBuf>,
    pub embedding_provider_config: Option<EmbeddingProviderConfig>,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagServeStarted {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub index_source: Option<String>,
    pub notes_root: Option<String>,
}

pub fn serve(
    opts: RagServeOptions,
    started: impl FnOnce(&RagServeStarted) -> Result<()>,
) -> Result<()> {
    serve_with_state(opts, None, started)
}

pub fn serve_with_note_viewer(
    opts: RagServeOptions,
    note_viewer: Arc<dyn NoteViewer>,
    started: impl FnOnce(&RagServeStarted) -> Result<()>,
) -> Result<()> {
    serve_with_state(opts, Some(note_viewer), started)
}

fn serve_with_state(
    opts: RagServeOptions,
    note_viewer: Option<Arc<dyn NoteViewer>>,
    started: impl FnOnce(&RagServeStarted) -> Result<()>,
) -> Result<()> {
    let embedding_provider_config = opts
        .embedding_provider_config
        .clone()
        .unwrap_or_else(default_embedding_provider_config);
    let mut state = AppState::with_embedding_provider_config(
        opts.db_path.clone(),
        opts.index_source.clone(),
        opts.notes_root.clone(),
        embedding_provider_config,
    );
    if let Some(note_viewer) = note_viewer {
        state = state.with_note_viewer(note_viewer);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to initialize RAG HTTP runtime")?;
    runtime.block_on(serve_until(opts, state, started, std::future::pending()))
}

pub(super) async fn serve_until<S>(
    opts: RagServeOptions,
    state: AppState,
    started: impl FnOnce(&RagServeStarted) -> Result<()>,
    shutdown: S,
) -> Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    let listener = tokio::net::TcpListener::bind((opts.host.as_str(), opts.port))
        .await
        .map_err(|err| anyhow::anyhow!("failed to bind {}:{}: {err}", opts.host, opts.port))?;
    let addr = listener.local_addr()?;
    let host = addr.ip().to_string();
    let port = addr.port();
    let url = format!("http://{}:{}/", url_host(addr.ip()), port);
    let start_index_on_launch = opts.index_source.is_some() || opts.notes_root.is_some();
    let started_event = RagServeStarted {
        url,
        host,
        port,
        db_path: display_path(&opts.db_path),
        index_source: opts.index_source.as_deref().map(display_path),
        notes_root: opts.notes_root.as_deref().map(display_path),
    };
    tracing::info!(
        event = "rag_api_serve_start",
        url = %started_event.url,
        db_path = %started_event.db_path,
        "serving RAG HTTP API"
    );
    started(&started_event)?;
    if start_index_on_launch {
        let progress = state.start_indexing();
        tracing::info!(
            event = "rag_api_startup_index",
            phase = progress.phase,
            current_step = progress.current_step,
            "started RAG index rebuild from configured serve source"
        );
    }
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
        .context("RAG HTTP server failed")
}

pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn url_host(host: std::net::IpAddr) -> String {
    match host {
        std::net::IpAddr::V4(host) => host.to_string(),
        std::net::IpAddr::V6(host) => format!("[{host}]"),
    }
}
