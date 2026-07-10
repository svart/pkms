//! Local retrieval models and indexing primitives for pkms.

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
pub use chunking::{CONTENT_HASH_PREFIX, ChunkNoteInput, chunk_note, content_hash};
#[cfg(feature = "fastembed")]
pub use embeddings::FastEmbeddingProvider;
pub use embeddings::{
    DEFAULT_EMBEDDING_MAX_BODY_CHARS, DEFAULT_FASTEMBED_BATCH_SIZE, DEFAULT_FASTEMBED_MODEL,
    DEFAULT_HASH_EMBEDDING_DIMENSION, EmbeddingProvider, EmbeddingProviderConfig,
    FASTEMBED_PROVIDER_NAME, HASH_PROVIDER_NAME, HashEmbeddingProvider, cosine_similarity,
    default_embedding_provider_config, embedding_provider_config, embedding_text, pack_vector,
    provider_from_config, unpack_vector,
};
pub use indexer::BackgroundIndexer;
pub use models::{
    ChunkRecord, DeleteEntityType, DeleteRecord, IndexPhase, IndexProgress, IndexStep,
    IngestSummary, LinkRecord, NoteRecord, RetrievalRecord, RetrieveMode, RetrieveRequest,
    RetrieveResponse, RetrieveResult, RetrieveWeights, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown,
    SearchRequest, SearchResponse, SearchResult, StatusResponse,
};
pub use ndjson::{NdjsonError, load_ndjson, parse_ndjson};
pub use org_export::{export_org_notes, export_org_notes_with_ignore};
pub use storage::RagIndex;
