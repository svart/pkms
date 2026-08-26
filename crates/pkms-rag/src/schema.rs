pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
    note_id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    aliases_json TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    content_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    chunk_id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    aliases_json TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    heading_path_json TEXT NOT NULL,
    heading_path_text TEXT NOT NULL,
    heading_level INTEGER NOT NULL,
    body TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    outgoing_ids_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    stale INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(note_id) REFERENCES notes(note_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS links (
    source_chunk_id TEXT NOT NULL,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT NOT NULL,
    link_text TEXT NOT NULL,
    PRIMARY KEY (source_chunk_id, target_note_id, link_text)
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_id UNINDEXED,
    note_id UNINDEXED,
    title,
    aliases,
    tags,
    heading_path,
    body,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    dimension INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    vector BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (chunk_id, model_name),
    FOREIGN KEY(chunk_id) REFERENCES chunks(chunk_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS index_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;
