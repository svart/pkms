use serde::{Deserialize, Deserializer, Serialize, de};

pub const SUPPORTED_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteRecord {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u8,
    pub note_id: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRecord {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u8,
    pub chunk_id: String,
    pub note_id: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub heading_path: Vec<String>,
    pub heading_level: u32,
    pub body: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default)]
    pub outgoing_ids: Vec<String>,
    pub updated_at: i64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u8,
    pub source_chunk_id: String,
    pub source_note_id: String,
    pub target_note_id: String,
    #[serde(default)]
    pub link_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteRecord {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u8,
    pub entity_type: DeleteEntityType,
    pub entity_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteEntityType {
    Note,
    Chunk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_type", rename_all = "snake_case")]
pub enum RetrievalRecord {
    Note(NoteRecord),
    Chunk(ChunkRecord),
    Link(LinkRecord),
    Delete(DeleteRecord),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestSummary {
    #[serde(default)]
    pub notes_seen: u64,
    #[serde(default)]
    pub notes_upserted: u64,
    #[serde(default)]
    pub chunks_seen: u64,
    #[serde(default)]
    pub chunks_upserted: u64,
    #[serde(default)]
    pub chunks_unchanged: u64,
    #[serde(default)]
    pub links_seen: u64,
    #[serde(default)]
    pub links_upserted: u64,
    #[serde(default)]
    pub deletes_seen: u64,
    #[serde(default)]
    pub notes_deleted: u64,
    #[serde(default)]
    pub chunks_deleted: u64,
    #[serde(default)]
    pub embeddings_computed: u64,
    #[serde(default)]
    pub embeddings_skipped: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexPhase {
    Idle,
    Loading,
    Indexing,
    Complete,
    Error,
}

impl IndexPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Indexing => "indexing",
            Self::Complete => "complete",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for IndexPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexStep {
    Idle,
    Starting,
    NoSource,
    ExportNotesRoot,
    ReadNdjson,
    InitEmbeddingModel,
    IngestRebuild,
    IngestRecords,
    EmbedChunks,
    CleanupStale,
    Complete,
    Error,
}

impl IndexStep {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::NoSource => "no-source",
            Self::ExportNotesRoot => "export-notes-root",
            Self::ReadNdjson => "read-ndjson",
            Self::InitEmbeddingModel => "init-embedding-model",
            Self::IngestRebuild => "ingest-rebuild",
            Self::IngestRecords => "ingest-records",
            Self::EmbedChunks => "embed-chunks",
            Self::CleanupStale => "cleanup-stale",
            Self::Complete => "complete",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for IndexStep {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexProgress {
    pub phase: IndexPhase,
    pub current_step: IndexStep,
    pub message: String,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub notes_root: Option<String>,
    #[serde(default)]
    pub total_records: u64,
    #[serde(default)]
    pub processed_records: u64,
    #[serde(default)]
    pub total_embeddings: u64,
    #[serde(default)]
    pub processed_embeddings: u64,
    #[serde(default)]
    pub started_at: Option<f64>,
    #[serde(default)]
    pub finished_at: Option<f64>,
    #[serde(default)]
    pub notes_seen: u64,
    #[serde(default)]
    pub notes_upserted: u64,
    #[serde(default)]
    pub chunks_seen: u64,
    #[serde(default)]
    pub chunks_upserted: u64,
    #[serde(default)]
    pub chunks_unchanged: u64,
    #[serde(default)]
    pub links_seen: u64,
    #[serde(default)]
    pub links_upserted: u64,
    #[serde(default)]
    pub deletes_seen: u64,
    #[serde(default)]
    pub notes_deleted: u64,
    #[serde(default)]
    pub chunks_deleted: u64,
    #[serde(default)]
    pub embeddings_computed: u64,
    #[serde(default)]
    pub embeddings_skipped: u64,
    #[serde(default)]
    pub error: Option<String>,
}

impl Default for IndexProgress {
    fn default() -> Self {
        Self {
            phase: IndexPhase::Idle,
            current_step: IndexStep::Idle,
            message: String::new(),
            source_path: None,
            notes_root: None,
            total_records: 0,
            processed_records: 0,
            total_embeddings: 0,
            processed_embeddings: 0,
            started_at: None,
            finished_at: None,
            notes_seen: 0,
            notes_upserted: 0,
            chunks_seen: 0,
            chunks_upserted: 0,
            chunks_unchanged: 0,
            links_seen: 0,
            links_upserted: 0,
            deletes_seen: 0,
            notes_deleted: 0,
            chunks_deleted: 0,
            embeddings_computed: 0,
            embeddings_skipped: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub schema_version: u8,
    pub notes: u64,
    pub chunks: u64,
    pub links: u64,
    pub stale_chunks: u64,
    pub fts_rows: u64,
    pub embeddings: u64,
    pub embedding_models: Vec<String>,
    pub db_path: String,
    pub last_indexed_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStatus {
    pub root: String,
    pub discovered_files: u64,
    pub indexable_notes: u64,
    pub indexed_notes: u64,
    pub empty_notes: u64,
    pub excluded_files: u64,
    pub missing_notes: u64,
    pub orphaned_index_notes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    #[serde(default)]
    pub bm25: f64,
    #[serde(default)]
    pub dense: f64,
    #[serde(default)]
    pub metadata: f64,
    #[serde(default)]
    pub graph: f64,
    #[serde(default, rename = "final")]
    pub final_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub heading_path: Vec<String>,
    pub heading_level: u32,
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
    pub scores: ScoreBreakdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetrieveWeights {
    #[serde(default = "default_bm25_weight")]
    pub bm25: f64,
    #[serde(default = "default_dense_weight")]
    pub dense: f64,
    #[serde(default = "default_metadata_weight")]
    pub metadata: f64,
    #[serde(default = "default_graph_weight")]
    pub graph: f64,
}

impl Default for RetrieveWeights {
    fn default() -> Self {
        Self {
            bm25: default_bm25_weight(),
            dense: default_dense_weight(),
            metadata: default_metadata_weight(),
            graph: default_graph_weight(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrieveMode {
    #[default]
    Hybrid,
    Bm25,
    Dense,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub mode: RetrieveMode,
    #[serde(default)]
    pub max_token_budget: Option<usize>,
    #[serde(default)]
    pub weights: RetrieveWeights,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrieveResult {
    #[serde(flatten)]
    pub result: SearchResult,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrieveResponse {
    pub query: String,
    pub mode: RetrieveMode,
    pub results: Vec<RetrieveResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagScope {
    Note,
    Heading,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagRecommendationEvidence {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub heading_path: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagRecommendation {
    pub tag: String,
    pub score: f64,
    pub support: usize,
    pub evidence: Vec<TagRecommendationEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagSourceRange {
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRecommendationRequest {
    pub query: String,
    pub scope: TagScope,
    pub target_note_id: String,
    #[serde(default)]
    pub target_range: Option<TagSourceRange>,
    #[serde(default)]
    pub existing_tags: Vec<String>,
    pub limit: usize,
    pub neighbor_limit: usize,
}

fn deserialize_schema_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u64::deserialize(deserializer)?;
    if version == u64::from(SUPPORTED_SCHEMA_VERSION) {
        Ok(SUPPORTED_SCHEMA_VERSION)
    } else {
        Err(de::Error::custom(format!(
            "unsupported schema_version {version}; expected {SUPPORTED_SCHEMA_VERSION}"
        )))
    }
}

fn default_limit() -> usize {
    10
}

fn default_bm25_weight() -> f64 {
    0.55
}

fn default_dense_weight() -> f64 {
    0.40
}

fn default_metadata_weight() -> f64 {
    0.03
}

fn default_graph_weight() -> f64 {
    0.02
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn models_parse_valid_tagged_note_record() {
        let record: RetrievalRecord = serde_json::from_value(json!({
            "schema_version": 1,
            "record_type": "note",
            "note_id": "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27",
            "path": "ops/media-library.org",
            "title": "Media Library Migration to Jellyfin",
            "updated_at": 1781686800,
            "content_hash": "sha256:note-media"
        }))
        .expect("valid note record parses");

        let RetrievalRecord::Note(note) = record else {
            panic!("expected note variant");
        };
        assert_eq!(note.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert!(note.aliases.is_empty());
        assert!(note.tags.is_empty());
    }

    #[test]
    fn models_serialize_record_type_tag() {
        let record = RetrievalRecord::Link(LinkRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            source_chunk_id: "source:chunk".to_string(),
            source_note_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            target_note_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
            link_text: String::new(),
        });

        let value = serde_json::to_value(record).expect("record serializes");

        assert_eq!(value["record_type"], "link");
        assert_eq!(value["schema_version"], 1);
    }

    #[test]
    fn models_reject_unsupported_schema_version() {
        let err = serde_json::from_value::<RetrievalRecord>(json!({
            "schema_version": 2,
            "record_type": "note",
            "note_id": "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27",
            "path": "ops/media-library.org",
            "title": "Media Library Migration to Jellyfin",
            "updated_at": 1781686800,
            "content_hash": "sha256:note-media"
        }))
        .expect_err("schema version 2 is rejected");

        assert!(err.to_string().contains("unsupported schema_version 2"));
    }

    #[test]
    fn models_reject_unknown_record_type() {
        let err = serde_json::from_value::<RetrievalRecord>(json!({
            "schema_version": 1,
            "record_type": "unknown",
            "id": "anything"
        }))
        .expect_err("unknown record type is rejected");

        assert!(err.to_string().contains("unknown variant"));
    }

    #[test]
    fn models_reject_missing_required_fields() {
        let err = serde_json::from_value::<RetrievalRecord>(json!({
            "schema_version": 1,
            "record_type": "chunk",
            "note_id": "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27",
            "path": "ops/media-library.org",
            "title": "Media Library Migration to Jellyfin",
            "heading_level": 2,
            "body": "body",
            "start_line": 1,
            "end_line": 2,
            "updated_at": 1781686800,
            "content_hash": "sha256:chunk-media"
        }))
        .expect_err("missing chunk_id is rejected");

        assert!(err.to_string().contains("missing field `chunk_id`"));
    }

    #[test]
    fn models_use_python_request_defaults() {
        let request: RetrieveRequest = serde_json::from_value(json!({
            "query": "agenda inspect tasks"
        }))
        .expect("request parses with defaults");

        assert_eq!(request.limit, 10);
        assert_eq!(request.mode, RetrieveMode::Hybrid);
        assert_eq!(request.weights, RetrieveWeights::default());
        assert_eq!(request.max_token_budget, None);
    }

    #[test]
    fn models_score_breakdown_uses_python_final_field_name() {
        let scores = ScoreBreakdown {
            final_score: 0.5,
            ..ScoreBreakdown::default()
        };

        let value = serde_json::to_value(scores).expect("scores serialize");
        assert_eq!(value["final"], 0.5);
        assert!(value.get("final_score").is_none());

        let decoded: ScoreBreakdown = serde_json::from_value(json!({ "bm25": 1.0, "final": 0.25 }))
            .expect("scores deserialize");
        assert_eq!(decoded.bm25, 1.0);
        assert_eq!(decoded.final_score, 0.25);
    }
}
