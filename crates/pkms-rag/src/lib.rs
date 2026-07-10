//! Local retrieval services and wire-contract types for pkms.

mod api;
mod chunking;
mod embeddings;
mod indexer;
mod models;
mod ndjson;
mod org_export;
mod retrieve;
mod schema;
mod storage;
mod web;

pub use api::{
    AppState, DEFAULT_RAG_DB, NoteViewer, NoteViewerMethod, NoteViewerRequest, NoteViewerResponse,
    RagServeOptions, RagServeStarted, router, serve, serve_with_note_viewer,
};
pub use embeddings::{
    DEFAULT_EMBEDDING_MAX_BODY_CHARS, DEFAULT_FASTEMBED_BATCH_SIZE, DEFAULT_FASTEMBED_MODEL,
    EmbeddingProvider, EmbeddingProviderConfig, embedding_provider_config, provider_from_config,
};
pub use indexer::BackgroundIndexer;
pub use models::{
    ChunkRecord, DeleteEntityType, DeleteRecord, IndexPhase, IndexProgress, IndexStep,
    IngestSummary, LinkRecord, NoteRecord, RetrievalRecord, RetrieveMode, RetrieveRequest,
    RetrieveResponse, RetrieveResult, RetrieveWeights, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown,
    SearchRequest, SearchResponse, SearchResult, StatusResponse,
};
pub use ndjson::{NdjsonError, load_ndjson};
pub use storage::RagIndex;
