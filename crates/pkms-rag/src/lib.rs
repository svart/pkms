//! Local retrieval models and indexing primitives for pkms.

pub mod chunking;
pub mod db;
pub mod embeddings;
pub mod models;
pub mod ndjson;
pub mod org_export;
pub mod schema;

pub use chunking::{CONTENT_HASH_PREFIX, ChunkNoteInput, chunk_note, content_hash};
pub use db::{build_fts_query, connect, dense_search, ingest_records, search, status};
pub use embeddings::{
    DEFAULT_EMBEDDING_MAX_BODY_CHARS, DEFAULT_HASH_EMBEDDING_DIMENSION, EmbeddingProvider,
    HashEmbeddingProvider, cosine_similarity, embedding_text, pack_vector, unpack_vector,
};
pub use models::{
    ChunkRecord, DeleteEntityType, DeleteRecord, IngestSummary, LinkRecord, NoteRecord,
    RetrievalRecord, RetrieveMode, RetrieveRequest, RetrieveResponse, RetrieveResult,
    RetrieveWeights, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown, SearchRequest, SearchResponse,
    SearchResult, StatusResponse,
};
pub use org_export::{export_org_notes, export_org_notes_with_ignore};
