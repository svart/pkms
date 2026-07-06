//! Local retrieval models and indexing primitives for pkms.

pub mod api;
pub mod chunking;
pub mod db;
pub mod embeddings;
pub mod indexer;
pub mod models;
pub mod ndjson;
pub mod org_export;
pub mod retrieve;
pub mod schema;
pub mod web;

pub use api::{
    AppState, DEFAULT_RAG_DB, NoteViewer, NoteViewerMethod, NoteViewerRequest, NoteViewerResponse,
    RagServeOptions, RagServeStarted, router, serve, serve_with_note_viewer,
};
pub use chunking::{CONTENT_HASH_PREFIX, ChunkNoteInput, chunk_note, content_hash};
pub use db::{build_fts_query, connect, dense_search, ingest_records, search, status};
#[cfg(feature = "fastembed")]
pub use embeddings::FastEmbeddingProvider;
pub use embeddings::{
    DEFAULT_EMBEDDING_MAX_BODY_CHARS, DEFAULT_FASTEMBED_BATCH_SIZE, DEFAULT_FASTEMBED_MODEL,
    DEFAULT_HASH_EMBEDDING_DIMENSION, EmbeddingProvider, EmbeddingProviderConfig,
    HashEmbeddingProvider, cosine_similarity, embedding_provider_config_from_env, embedding_text,
    pack_vector, provider_from_config, provider_from_env, unpack_vector,
};
pub use indexer::BackgroundIndexer;
pub use models::{
    ChunkRecord, DeleteEntityType, DeleteRecord, IndexProgress, IngestSummary, LinkRecord,
    NoteRecord, RetrievalRecord, RetrieveMode, RetrieveRequest, RetrieveResponse, RetrieveResult,
    RetrieveWeights, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown, SearchRequest, SearchResponse,
    SearchResult, StatusResponse,
};
pub use org_export::{export_org_notes, export_org_notes_with_ignore};
pub use retrieve::{retrieve, retrieve_results};
