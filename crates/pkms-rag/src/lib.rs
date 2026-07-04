//! Local retrieval models and indexing primitives for pkms.

pub mod db;
pub mod embeddings;
pub mod models;
pub mod ndjson;
pub mod schema;

pub use db::{connect, status};
pub use embeddings::{cosine_similarity, pack_vector, unpack_vector};
pub use models::{
    ChunkRecord, DeleteEntityType, DeleteRecord, IngestSummary, LinkRecord, NoteRecord,
    RetrievalRecord, RetrieveMode, RetrieveRequest, RetrieveResponse, RetrieveResult,
    RetrieveWeights, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown, SearchRequest, SearchResponse,
    SearchResult, StatusResponse,
};
