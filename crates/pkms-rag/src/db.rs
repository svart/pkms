use std::{
    cmp::Ordering,
    collections::HashSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::{
    embeddings::{
        DEFAULT_EMBEDDING_MAX_BODY_CHARS, EmbeddingProvider, cosine_similarity,
        embedding_text_with_max_body_chars, pack_vector, unpack_vector,
    },
    models::{
        ChunkRecord, DeleteEntityType, DeleteRecord, IngestSummary, LinkRecord, RetrievalRecord,
        SUPPORTED_SCHEMA_VERSION, ScoreBreakdown, SearchResult, StatusResponse,
    },
    schema::SCHEMA_SQL,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IngestProgress {
    pub current_step: String,
    pub message: String,
    pub total_records: u64,
    pub processed_records: u64,
    pub total_embeddings: u64,
    pub processed_embeddings: u64,
    pub summary: IngestSummary,
}

pub fn connect(db_path: impl AsRef<Path>) -> Result<Connection> {
    let db_path = db_path.as_ref();
    create_parent_dir(db_path)?;
    let conn = Connection::open(db_path).with_context(|| {
        format!(
            "failed to open RAG SQLite database {}",
            display_path(db_path)
        )
    })?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("failed to enable SQLite foreign keys")?;
    ensure_schema(&conn)?;
    Ok(conn)
}

pub fn remove_index_files(db_path: impl AsRef<Path>) -> Result<()> {
    let db_path = db_path.as_ref();
    for path in sqlite_index_files(db_path) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to remove RAG SQLite index file {}",
                        display_path(&path)
                    )
                });
            }
        }
    }
    Ok(())
}

pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)
        .context("failed to initialize RAG SQLite schema")
}

pub fn ingest_records(
    conn: &mut Connection,
    records: &[RetrievalRecord],
    embedding_provider: &dyn EmbeddingProvider,
    full_rebuild: bool,
) -> Result<IngestSummary> {
    ingest_records_with_progress(
        conn,
        records,
        embedding_provider,
        full_rebuild,
        DEFAULT_EMBEDDING_MAX_BODY_CHARS,
        |_| {},
    )
}

pub(crate) fn ingest_records_with_progress(
    conn: &mut Connection,
    records: &[RetrievalRecord],
    embedding_provider: &dyn EmbeddingProvider,
    full_rebuild: bool,
    embedding_max_body_chars: usize,
    mut on_progress: impl FnMut(&IngestProgress),
) -> Result<IngestSummary> {
    let tx = conn
        .transaction()
        .context("failed to start RAG ingest transaction")?;
    let mut summary = IngestSummary::default();
    let mut changed_chunks = Vec::new();
    let mut seen_note_ids = HashSet::new();
    let total_records = records.len() as u64;

    if full_rebuild {
        tx.execute("UPDATE chunks SET stale = 1", [])
            .context("failed to mark chunks stale for full rebuild")?;
    }

    on_progress(&IngestProgress {
        current_step: "ingest-records".to_string(),
        message: format!("Processing {total_records} records."),
        total_records,
        processed_records: 0,
        total_embeddings: 0,
        processed_embeddings: 0,
        summary: summary.clone(),
    });

    for (index, record) in records.iter().enumerate() {
        match record {
            RetrievalRecord::Note(record) => {
                seen_note_ids.insert(record.note_id.clone());
                summary.notes_seen += 1;
                if upsert_note(&tx, record)? {
                    summary.notes_upserted += 1;
                }
            }
            RetrievalRecord::Chunk(record) => {
                seen_note_ids.insert(record.note_id.clone());
                summary.chunks_seen += 1;
                if upsert_chunk(&tx, record)? {
                    summary.chunks_upserted += 1;
                    changed_chunks.push(record.clone());
                } else {
                    summary.chunks_unchanged += 1;
                }
            }
            RetrievalRecord::Link(record) => {
                summary.links_seen += 1;
                summary.links_upserted += upsert_link(&tx, record)?;
            }
            RetrievalRecord::Delete(record) => {
                summary.deletes_seen += 1;
                let (notes_deleted, chunks_deleted) = delete_record(&tx, record)?;
                summary.notes_deleted += notes_deleted;
                summary.chunks_deleted += chunks_deleted;
            }
        }
        on_progress(&IngestProgress {
            current_step: "ingest-records".to_string(),
            message: format!("Processing {total_records} records."),
            total_records,
            processed_records: index as u64 + 1,
            total_embeddings: 0,
            processed_embeddings: 0,
            summary: summary.clone(),
        });
    }

    let (embeddings_computed, embeddings_skipped) = refresh_embeddings_with_progress(
        &tx,
        &changed_chunks,
        embedding_provider,
        embedding_max_body_chars,
        |processed_embeddings, total_embeddings, embeddings_skipped| {
            let mut progress_summary = summary.clone();
            progress_summary.embeddings_computed = processed_embeddings;
            progress_summary.embeddings_skipped = embeddings_skipped;
            on_progress(&IngestProgress {
                current_step: "embed-chunks".to_string(),
                message: format!("Embedding {total_embeddings} changed chunks."),
                total_records,
                processed_records: total_records,
                total_embeddings,
                processed_embeddings,
                summary: progress_summary,
            });
        },
    )?;
    summary.embeddings_computed += embeddings_computed;
    summary.embeddings_skipped += embeddings_skipped;

    if full_rebuild {
        summary.chunks_deleted += delete_stale_chunks(&tx)?;
        summary.notes_deleted += delete_missing_notes(&tx, &seen_note_ids)?;
        on_progress(&IngestProgress {
            current_step: "cleanup-stale".to_string(),
            message: "Removing stale index rows.".to_string(),
            total_records,
            processed_records: total_records,
            total_embeddings: embeddings_computed,
            processed_embeddings: embeddings_computed,
            summary: summary.clone(),
        });
    }

    tx.commit()
        .context("failed to commit RAG ingest transaction")?;
    Ok(summary)
}

pub fn status(conn: &Connection, db_path: impl AsRef<Path>) -> Result<StatusResponse> {
    Ok(StatusResponse {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        notes: count(conn, "SELECT COUNT(*) FROM notes")?,
        chunks: count(conn, "SELECT COUNT(*) FROM chunks")?,
        links: count(conn, "SELECT COUNT(*) FROM links")?,
        stale_chunks: count(conn, "SELECT COUNT(*) FROM chunks WHERE stale = 1")?,
        fts_rows: count(conn, "SELECT COUNT(*) FROM chunks_fts")?,
        embeddings: count(conn, "SELECT COUNT(*) FROM chunk_embeddings")?,
        embedding_models: embedding_models(conn)?,
        db_path: display_path(db_path.as_ref()),
    })
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let fts_query = build_fts_query(query);
    if fts_query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                c.chunk_id,
                c.note_id,
                c.title,
                c.path,
                c.aliases_json,
                c.tags_json,
                c.heading_path_json,
                c.heading_level,
                c.start_line,
                c.end_line,
                c.body,
                bm25(chunks_fts) AS raw_bm25
            FROM chunks_fts
            JOIN chunks c ON c.chunk_id = chunks_fts.chunk_id
            WHERE chunks_fts MATCH ?
              AND c.stale = 0
            ORDER BY raw_bm25 ASC
            LIMIT ?
            "#,
        )
        .context("failed to prepare RAG search query")?;
    let rows = stmt
        .query_map(params![fts_query, limit as i64], row_to_search_result)
        .context("failed to run RAG search query")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read RAG search rows")
}

pub fn dense_search(
    conn: &Connection,
    query: &str,
    limit: usize,
    embedding_provider: &dyn EmbeddingProvider,
) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let query_vectors = embedding_provider
        .embed(&[query.to_string()])
        .context("failed to embed dense search query")?;
    anyhow::ensure!(
        query_vectors.len() == 1,
        "embedding provider returned {} query vectors; expected 1",
        query_vectors.len()
    );
    let query_vector = &query_vectors[0];
    anyhow::ensure!(
        query_vector.len() == embedding_provider.dimension(),
        "embedding provider returned query dimension {} for model {}; expected {}",
        query_vector.len(),
        embedding_provider.model_name(),
        embedding_provider.dimension()
    );

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                c.chunk_id,
                c.note_id,
                c.title,
                c.path,
                c.aliases_json,
                c.tags_json,
                c.heading_path_json,
                c.heading_level,
                c.start_line,
                c.end_line,
                c.body,
                e.vector
            FROM chunk_embeddings e
            JOIN chunks c ON c.chunk_id = e.chunk_id
            WHERE e.model_name = ?
              AND e.dimension = ?
              AND c.stale = 0
            "#,
        )
        .context("failed to prepare RAG dense search query")?;
    let mut rows = stmt
        .query(params![
            embedding_provider.model_name(),
            embedding_provider.dimension() as i64
        ])
        .context("failed to run RAG dense search query")?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().context("failed to read RAG dense search row")? {
        let mut result = row_to_stored_search_result(row, ScoreBreakdown::default())
            .context("failed to read RAG dense search result")?;
        let vector_blob = row
            .get::<_, Vec<u8>>("vector")
            .context("failed to read RAG dense search vector blob")?;
        let stored_vector = unpack_vector(&vector_blob).with_context(|| {
            format!(
                "failed to unpack dense vector for chunk {}",
                result.chunk_id
            )
        })?;
        anyhow::ensure!(
            stored_vector.len() == embedding_provider.dimension(),
            "stored dense vector for chunk {} has dimension {}; expected {}",
            result.chunk_id,
            stored_vector.len(),
            embedding_provider.dimension()
        );

        let score = f64::from(cosine_similarity(query_vector, &stored_vector).max(0.0));
        result.scores = ScoreBreakdown {
            dense: score,
            final_score: score,
            ..ScoreBreakdown::default()
        };
        results.push(result);
    }

    results.sort_by(|left, right| {
        right
            .scores
            .dense
            .partial_cmp(&left.scores.dense)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    results.truncate(limit);
    Ok(results)
}

pub fn build_fts_query(query: &str) -> String {
    let mut parts = Vec::new();
    let mut chars = query.chars().peekable();
    loop {
        while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
            chars.next();
        }
        let Some(first) = chars.next() else {
            break;
        };
        let mut value = String::new();
        if first == '"' {
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                value.push(ch);
            }
        } else {
            value.push(first);
            while let Some(ch) = chars.peek().copied() {
                if ch.is_whitespace() {
                    break;
                }
                value.push(ch);
                chars.next();
            }
        }
        let value = value.trim();
        if !value.is_empty() {
            parts.push(format!("\"{}\"", value.replace('"', "\"\"")));
        }
    }
    parts.join(" OR ")
}

fn row_to_search_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchResult> {
    let raw_bm25 = row.get::<_, f64>("raw_bm25")?;
    let magnitude = raw_bm25.abs();
    let score = magnitude / (1.0 + magnitude);
    row_to_stored_search_result(
        row,
        ScoreBreakdown {
            bm25: score,
            final_score: score,
            ..ScoreBreakdown::default()
        },
    )
}

pub(crate) fn row_to_stored_search_result(
    row: &rusqlite::Row<'_>,
    scores: ScoreBreakdown,
) -> rusqlite::Result<SearchResult> {
    Ok(SearchResult {
        chunk_id: row.get("chunk_id")?,
        uuid: row.get("note_id")?,
        title: row.get("title")?,
        path: row.get("path")?,
        aliases: json_column(row, "aliases_json")?,
        tags: json_column(row, "tags_json")?,
        heading_path: json_column(row, "heading_path_json")?,
        heading_level: row.get("heading_level")?,
        start_line: row.get("start_line")?,
        end_line: row.get("end_line")?,
        text: row.get("body")?,
        scores,
    })
}

fn json_column<T>(row: &rusqlite::Row<'_>, column: &str) -> rusqlite::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let value = row.get::<_, String>(column)?;
    serde_json::from_str(&value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(err),
        )
    })
}

fn upsert_note(tx: &Transaction<'_>, record: &crate::models::NoteRecord) -> Result<bool> {
    let aliases_json =
        serde_json::to_string(&record.aliases).context("failed to encode aliases")?;
    let tags_json = serde_json::to_string(&record.tags).context("failed to encode tags")?;
    let existing_hash = tx
        .query_row(
            "SELECT content_hash FROM notes WHERE note_id = ?",
            [&record.note_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to query existing note hash")?;

    if existing_hash.as_deref() == Some(&record.content_hash) {
        tx.execute(
            r#"
            UPDATE notes
            SET path = ?,
                title = ?,
                aliases_json = ?,
                tags_json = ?,
                updated_at = ?,
                content_hash = ?
            WHERE note_id = ?
            "#,
            params![
                record.path,
                record.title,
                aliases_json,
                tags_json,
                record.updated_at,
                record.content_hash,
                record.note_id
            ],
        )
        .context("failed to refresh unchanged note metadata")?;
        return Ok(false);
    }

    tx.execute(
        r#"
        INSERT INTO notes (
            note_id, path, title, aliases_json, tags_json, updated_at, content_hash
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(note_id) DO UPDATE SET
            path = excluded.path,
            title = excluded.title,
            aliases_json = excluded.aliases_json,
            tags_json = excluded.tags_json,
            updated_at = excluded.updated_at,
            content_hash = excluded.content_hash
        "#,
        params![
            record.note_id,
            record.path,
            record.title,
            aliases_json,
            tags_json,
            record.updated_at,
            record.content_hash
        ],
    )
    .context("failed to upsert note")?;
    Ok(true)
}

fn upsert_chunk(tx: &Transaction<'_>, record: &ChunkRecord) -> Result<bool> {
    let aliases_json =
        serde_json::to_string(&record.aliases).context("failed to encode aliases")?;
    let tags_json = serde_json::to_string(&record.tags).context("failed to encode tags")?;
    let heading_path_json =
        serde_json::to_string(&record.heading_path).context("failed to encode heading path")?;
    let outgoing_ids_json =
        serde_json::to_string(&record.outgoing_ids).context("failed to encode outgoing IDs")?;
    let heading_path_text = record.heading_path.join(" / ");
    let existing_hash = tx
        .query_row(
            "SELECT content_hash FROM chunks WHERE chunk_id = ?",
            [&record.chunk_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to query existing chunk hash")?;

    if existing_hash.as_deref() == Some(&record.content_hash) {
        tx.execute(
            r#"
            UPDATE chunks
            SET note_id = ?,
                path = ?,
                title = ?,
                aliases_json = ?,
                tags_json = ?,
                heading_path_json = ?,
                heading_path_text = ?,
                heading_level = ?,
                body = ?,
                start_line = ?,
                end_line = ?,
                outgoing_ids_json = ?,
                updated_at = ?,
                content_hash = ?,
                stale = 0
            WHERE chunk_id = ?
            "#,
            params![
                record.note_id,
                record.path,
                record.title,
                aliases_json,
                tags_json,
                heading_path_json,
                heading_path_text,
                record.heading_level,
                record.body,
                record.start_line,
                record.end_line,
                outgoing_ids_json,
                record.updated_at,
                record.content_hash,
                record.chunk_id
            ],
        )
        .context("failed to refresh unchanged chunk metadata")?;
        refresh_fts(tx, record)?;
        sync_chunk_links(tx, record)?;
        return Ok(false);
    }

    tx.execute(
        r#"
        INSERT INTO chunks (
            chunk_id, note_id, path, title, aliases_json, tags_json,
            heading_path_json, heading_path_text, heading_level, body,
            start_line, end_line, outgoing_ids_json, updated_at, content_hash, stale
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
        ON CONFLICT(chunk_id) DO UPDATE SET
            note_id = excluded.note_id,
            path = excluded.path,
            title = excluded.title,
            aliases_json = excluded.aliases_json,
            tags_json = excluded.tags_json,
            heading_path_json = excluded.heading_path_json,
            heading_path_text = excluded.heading_path_text,
            heading_level = excluded.heading_level,
            body = excluded.body,
            start_line = excluded.start_line,
            end_line = excluded.end_line,
            outgoing_ids_json = excluded.outgoing_ids_json,
            updated_at = excluded.updated_at,
            content_hash = excluded.content_hash,
            stale = 0
        "#,
        params![
            record.chunk_id,
            record.note_id,
            record.path,
            record.title,
            aliases_json,
            tags_json,
            heading_path_json,
            heading_path_text,
            record.heading_level,
            record.body,
            record.start_line,
            record.end_line,
            outgoing_ids_json,
            record.updated_at,
            record.content_hash
        ],
    )
    .context("failed to upsert chunk")?;
    refresh_fts(tx, record)?;
    sync_chunk_links(tx, record)?;
    Ok(true)
}

fn refresh_fts(tx: &Transaction<'_>, record: &ChunkRecord) -> Result<()> {
    tx.execute(
        "DELETE FROM chunks_fts WHERE chunk_id = ?",
        [&record.chunk_id],
    )
    .context("failed to delete stale FTS row")?;
    tx.execute(
        r#"
        INSERT INTO chunks_fts (
            chunk_id, note_id, title, aliases, tags, heading_path, body
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
        params![
            record.chunk_id,
            record.note_id,
            record.title,
            record.aliases.join(" "),
            record.tags.join(" "),
            record.heading_path.join(" / "),
            record.body
        ],
    )
    .context("failed to insert FTS row")?;
    Ok(())
}

fn upsert_link(tx: &Transaction<'_>, record: &LinkRecord) -> Result<u64> {
    if record.link_text.is_empty() {
        let exists = tx
            .query_row(
                r#"
                SELECT 1
                FROM links
                WHERE source_chunk_id = ?
                  AND target_note_id = ?
                LIMIT 1
                "#,
                params![record.source_chunk_id, record.target_note_id],
                |_| Ok(()),
            )
            .optional()
            .context("failed to query existing link")?
            .is_some();
        if exists {
            return Ok(0);
        }
    } else {
        tx.execute(
            r#"
            DELETE FROM links
            WHERE source_chunk_id = ?
              AND target_note_id = ?
              AND link_text = ''
            "#,
            params![record.source_chunk_id, record.target_note_id],
        )
        .context("failed to delete implicit link before explicit link upsert")?;
    }

    let rows = tx
        .execute(
            r#"
            INSERT OR IGNORE INTO links (
                source_chunk_id, source_note_id, target_note_id, link_text
            ) VALUES (?, ?, ?, ?)
            "#,
            params![
                record.source_chunk_id,
                record.source_note_id,
                record.target_note_id,
                record.link_text
            ],
        )
        .context("failed to upsert link")?;
    Ok(rows as u64)
}

fn sync_chunk_links(tx: &Transaction<'_>, record: &ChunkRecord) -> Result<u64> {
    tx.execute(
        "DELETE FROM links WHERE source_chunk_id = ?",
        [&record.chunk_id],
    )
    .context("failed to clear chunk links")?;
    let mut inserted = 0;
    let mut outgoing_ids = record.outgoing_ids.clone();
    outgoing_ids.sort();
    outgoing_ids.dedup();
    for target_note_id in outgoing_ids {
        if target_note_id == record.note_id {
            continue;
        }
        inserted += upsert_link(
            tx,
            &LinkRecord {
                schema_version: record.schema_version,
                source_chunk_id: record.chunk_id.clone(),
                source_note_id: record.note_id.clone(),
                target_note_id,
                link_text: String::new(),
            },
        )?;
    }
    Ok(inserted)
}

fn delete_record(tx: &Transaction<'_>, record: &DeleteRecord) -> Result<(u64, u64)> {
    match record.entity_type {
        DeleteEntityType::Note => delete_note(tx, &record.entity_id),
        DeleteEntityType::Chunk => delete_chunk(tx, &record.entity_id).map(|chunks| (0, chunks)),
    }
}

fn delete_note(tx: &Transaction<'_>, note_id: &str) -> Result<(u64, u64)> {
    let chunk_ids = chunk_ids_for_note(tx, note_id)?;
    for chunk_id in &chunk_ids {
        delete_chunk_side_tables(tx, chunk_id)?;
    }
    tx.execute(
        "DELETE FROM links WHERE source_note_id = ? OR target_note_id = ?",
        params![note_id, note_id],
    )
    .context("failed to delete note links")?;
    let notes_deleted = tx
        .execute("DELETE FROM notes WHERE note_id = ?", [note_id])
        .context("failed to delete note")?;
    Ok((notes_deleted as u64, chunk_ids.len() as u64))
}

fn delete_chunk(tx: &Transaction<'_>, chunk_id: &str) -> Result<u64> {
    delete_chunk_side_tables(tx, chunk_id)?;
    let chunks_deleted = tx
        .execute("DELETE FROM chunks WHERE chunk_id = ?", [chunk_id])
        .context("failed to delete chunk")?;
    Ok(chunks_deleted as u64)
}

fn delete_chunk_side_tables(tx: &Transaction<'_>, chunk_id: &str) -> Result<()> {
    tx.execute("DELETE FROM chunks_fts WHERE chunk_id = ?", [chunk_id])
        .context("failed to delete chunk FTS rows")?;
    tx.execute(
        "DELETE FROM chunk_embeddings WHERE chunk_id = ?",
        [chunk_id],
    )
    .context("failed to delete chunk embeddings")?;
    tx.execute("DELETE FROM links WHERE source_chunk_id = ?", [chunk_id])
        .context("failed to delete chunk links")?;
    Ok(())
}

fn delete_stale_chunks(tx: &Transaction<'_>) -> Result<u64> {
    let chunk_ids = query_strings(tx, "SELECT chunk_id FROM chunks WHERE stale = 1")?;
    for chunk_id in &chunk_ids {
        delete_chunk_side_tables(tx, chunk_id)?;
    }
    let chunks_deleted = tx
        .execute("DELETE FROM chunks WHERE stale = 1", [])
        .context("failed to delete stale chunks")?;
    Ok(chunks_deleted as u64)
}

fn delete_missing_notes(tx: &Transaction<'_>, seen_note_ids: &HashSet<String>) -> Result<u64> {
    if seen_note_ids.is_empty() {
        tx.execute("DELETE FROM links", [])
            .context("failed to delete links for empty rebuild")?;
        let notes_deleted = tx
            .execute("DELETE FROM notes", [])
            .context("failed to delete notes for empty rebuild")?;
        return Ok(notes_deleted as u64);
    }

    let mut notes_deleted = 0;
    let note_ids = query_strings(tx, "SELECT note_id FROM notes")?;
    for note_id in note_ids {
        if seen_note_ids.contains(&note_id) {
            continue;
        }
        notes_deleted += delete_note(tx, &note_id)?.0;
    }
    Ok(notes_deleted)
}

fn refresh_embeddings_with_progress(
    tx: &Transaction<'_>,
    chunks: &[ChunkRecord],
    provider: &dyn EmbeddingProvider,
    embedding_max_body_chars: usize,
    mut on_progress: impl FnMut(u64, u64, u64),
) -> Result<(u64, u64)> {
    let mut chunks_to_embed = Vec::new();
    let mut skipped = 0;
    for chunk in chunks {
        let existing = tx
            .query_row(
                r#"
                SELECT content_hash, dimension
                FROM chunk_embeddings
                WHERE chunk_id = ? AND model_name = ?
                "#,
                params![chunk.chunk_id, provider.model_name()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .context("failed to query existing embedding")?;
        if let Some((content_hash, dimension)) = existing
            && content_hash == chunk.content_hash
            && dimension == provider.dimension() as i64
        {
            skipped += 1;
            continue;
        }
        chunks_to_embed.push(chunk.clone());
    }

    let total_embeddings = chunks_to_embed.len() as u64;
    on_progress(0, total_embeddings, skipped);

    if chunks_to_embed.is_empty() {
        return Ok((0, skipped));
    }

    let created_at = unix_timestamp()?;
    let batch_size = provider
        .preferred_batch_size()
        .filter(|batch_size| *batch_size > 0)
        .unwrap_or(chunks_to_embed.len());
    let mut processed_embeddings = 0;
    for chunk_batch in chunks_to_embed.chunks(batch_size) {
        let texts = chunk_batch
            .iter()
            .map(|chunk| embedding_text_with_max_body_chars(chunk, embedding_max_body_chars))
            .collect::<Vec<_>>();
        let vectors = provider.embed(&texts).context("failed to embed chunks")?;
        anyhow::ensure!(
            vectors.len() == chunk_batch.len(),
            "embedding provider returned {} vectors for {} chunks",
            vectors.len(),
            chunk_batch.len()
        );
        for (chunk, vector) in chunk_batch.iter().zip(vectors) {
            anyhow::ensure!(
                vector.len() == provider.dimension(),
                "embedding provider returned dimension {} for model {}; expected {}",
                vector.len(),
                provider.model_name(),
                provider.dimension()
            );
            tx.execute(
                r#"
                INSERT INTO chunk_embeddings (
                    chunk_id, model_name, dimension, content_hash, vector, created_at
                ) VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(chunk_id, model_name) DO UPDATE SET
                    dimension = excluded.dimension,
                    content_hash = excluded.content_hash,
                    vector = excluded.vector,
                    created_at = excluded.created_at
                "#,
                params![
                    chunk.chunk_id,
                    provider.model_name(),
                    provider.dimension() as i64,
                    chunk.content_hash,
                    pack_vector(&vector),
                    created_at
                ],
            )
            .context("failed to upsert chunk embedding")?;
        }
        processed_embeddings += chunk_batch.len() as u64;
        on_progress(processed_embeddings, total_embeddings, skipped);
    }

    Ok((total_embeddings, skipped))
}

fn query_strings(tx: &Transaction<'_>, query: &str) -> Result<Vec<String>> {
    let mut stmt = tx
        .prepare(query)
        .context("failed to prepare string query")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("failed to query string rows")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read string rows")
}

fn chunk_ids_for_note(tx: &Transaction<'_>, note_id: &str) -> Result<Vec<String>> {
    let mut stmt = tx
        .prepare("SELECT chunk_id FROM chunks WHERE note_id = ?")
        .context("failed to prepare note chunk query")?;
    let rows = stmt
        .query_map([note_id], |row| row.get::<_, String>(0))
        .context("failed to query note chunks")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read note chunks")
}

fn unix_timestamp() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs() as i64)
}

fn count(conn: &Connection, query: &str) -> Result<u64> {
    let count = conn
        .query_row(query, [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("failed to run status count query: {query}"))?;
    u64::try_from(count).context("SQLite count was negative")
}

fn embedding_models(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT model_name FROM chunk_embeddings ORDER BY model_name")
        .context("failed to prepare embedding model status query")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("failed to query embedding models")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read embedding model rows")
}

fn create_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create RAG SQLite database directory {}",
                display_path(parent)
            )
        })?;
    }
    Ok(())
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn sqlite_index_files(db_path: &Path) -> [PathBuf; 4] {
    [
        db_path.to_path_buf(),
        sqlite_sidecar_path(db_path, "-journal"),
        sqlite_sidecar_path(db_path, "-wal"),
        sqlite_sidecar_path(db_path, "-shm"),
    ]
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(db_path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{embeddings::HashEmbeddingProvider, models::NoteRecord, ndjson::load_ndjson};
    use std::{path::PathBuf, sync::Mutex};

    fn db_path() -> (tempfile::TempDir, PathBuf) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("nested").join("rag.sqlite3");
        (tempdir, path)
    }

    #[test]
    fn remove_index_files_deletes_sqlite_database_and_sidecars() {
        let (_tempdir, path) = db_path();
        let journal_path = sqlite_sidecar_path(&path, "-journal");
        let wal_path = sqlite_sidecar_path(&path, "-wal");
        let shm_path = sqlite_sidecar_path(&path, "-shm");
        fs::create_dir_all(path.parent().expect("path has parent")).expect("parent creates");
        fs::write(&path, b"db").expect("db writes");
        fs::write(&journal_path, b"journal").expect("journal writes");
        fs::write(&wal_path, b"wal").expect("wal writes");
        fs::write(&shm_path, b"shm").expect("shm writes");

        remove_index_files(&path).expect("index files remove");

        assert!(!path.exists());
        assert!(!journal_path.exists());
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());
        remove_index_files(&path).expect("missing index files are ignored");
    }

    #[test]
    fn db_connect_creates_parent_directory_and_schema() {
        let (_tempdir, path) = db_path();

        let conn = connect(&path).expect("connect initializes database");

        assert!(path.exists());
        assert_eq!(table_count(&conn, "notes"), 1);
        assert_eq!(table_count(&conn, "chunks"), 1);
        assert_eq!(table_count(&conn, "links"), 1);
        assert_eq!(table_count(&conn, "chunk_embeddings"), 1);
    }

    #[test]
    fn db_connect_enables_foreign_keys() {
        let (_tempdir, path) = db_path();
        let conn = connect(path).expect("connect initializes database");

        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign key pragma reads");

        assert_eq!(enabled, 1);
    }

    #[test]
    fn db_status_reports_empty_database() {
        let (_tempdir, path) = db_path();
        let conn = connect(&path).expect("connect initializes database");

        let current = status(&conn, &path).expect("status reads");

        assert_eq!(current.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(current.notes, 0);
        assert_eq!(current.chunks, 0);
        assert_eq!(current.links, 0);
        assert_eq!(current.stale_chunks, 0);
        assert_eq!(current.fts_rows, 0);
        assert_eq!(current.embeddings, 0);
        assert!(current.embedding_models.is_empty());
        assert_eq!(current.db_path, path.to_string_lossy().into_owned());
    }

    #[test]
    fn db_schema_creates_queryable_fts_table() {
        let (_tempdir, path) = db_path();
        let conn = connect(path).expect("connect initializes database");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| row.get(0))
            .expect("fts table is queryable");

        assert_eq!(rows, 0);
    }

    fn table_count(conn: &Connection, name: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type IN ('table', 'virtual') AND name = ?",
            [name],
            |row| row.get(0),
        )
        .expect("table count reads")
    }

    #[test]
    fn ingest_fixture_populates_status_and_embeddings() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let records = fixture_records();
        let provider = HashEmbeddingProvider::default();

        let summary =
            ingest_records(&mut conn, &records, &provider, false).expect("fixture ingests");
        let current = status(&conn, &path).expect("status reads");

        assert_eq!(summary.notes_seen, 2);
        assert_eq!(summary.notes_upserted, 2);
        assert_eq!(summary.chunks_seen, 2);
        assert_eq!(summary.chunks_upserted, 2);
        assert_eq!(summary.links_seen, 1);
        assert_eq!(summary.links_upserted, 1);
        assert_eq!(summary.embeddings_computed, 2);
        assert_eq!(current.notes, 2);
        assert_eq!(current.chunks, 2);
        assert_eq!(current.links, 1);
        assert_eq!(current.fts_rows, 2);
        assert_eq!(current.embeddings, 2);
        assert_eq!(current.embedding_models, vec!["hashing-v1"]);
    }

    #[test]
    fn ingest_is_idempotent_for_unchanged_fixture() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let records = fixture_records();
        let provider = HashEmbeddingProvider::default();

        ingest_records(&mut conn, &records, &provider, false).expect("fixture ingests");
        let second =
            ingest_records(&mut conn, &records, &provider, false).expect("fixture reingests");
        let current = status(&conn, &path).expect("status reads");

        assert_eq!(second.notes_upserted, 0);
        assert_eq!(second.chunks_upserted, 0);
        assert_eq!(second.chunks_unchanged, 2);
        assert_eq!(second.embeddings_computed, 0);
        assert_eq!(second.embeddings_skipped, 0);
        assert_eq!(current.notes, 2);
        assert_eq!(current.chunks, 2);
        assert_eq!(current.links, 1);
        assert_eq!(current.fts_rows, 2);
        assert_eq!(current.embeddings, 2);
    }

    #[test]
    fn ingest_full_rebuild_removes_records_missing_from_new_export() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let records = fixture_records();
        let provider = HashEmbeddingProvider::default();

        ingest_records(&mut conn, &records, &provider, true).expect("fixture ingests");
        let summary = ingest_records(&mut conn, &records[..2], &provider, true)
            .expect("partial fixture ingests");
        let current = status(&conn, &path).expect("status reads");

        assert_eq!(summary.chunks_deleted, 1);
        assert_eq!(summary.notes_deleted, 1);
        assert_eq!(current.notes, 1);
        assert_eq!(current.chunks, 1);
        assert_eq!(current.links, 0);
        assert_eq!(current.fts_rows, 1);
        assert_eq!(current.embeddings, 1);
    }

    #[test]
    fn ingest_refreshes_metadata_when_content_hash_is_unchanged() {
        let (_tempdir, _path) = db_path();
        let mut conn = connect(_path).expect("connect initializes database");
        let records = fixture_records();
        let mut renamed_records = records.clone();
        for record in &mut renamed_records {
            match record {
                RetrievalRecord::Note(note) if note.path == "ops/media-library.org" => {
                    note.path = "ops/renamed-media-library.org".to_string();
                    note.updated_at += 100;
                }
                RetrievalRecord::Chunk(chunk) if chunk.path == "ops/media-library.org" => {
                    chunk.path = "ops/renamed-media-library.org".to_string();
                    chunk.updated_at += 100;
                }
                _ => {}
            }
        }
        let provider = HashEmbeddingProvider::default();

        ingest_records(&mut conn, &records, &provider, true).expect("fixture ingests");
        ingest_records(&mut conn, &renamed_records, &provider, true)
            .expect("renamed fixture ingests");

        let path: String = conn
            .query_row(
                "SELECT path FROM chunks WHERE chunk_id = ?",
                ["c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27:seerr-jellyfin-links:abc123"],
                |row| row.get(0),
            )
            .expect("chunk path reads");
        assert_eq!(path, "ops/renamed-media-library.org");
    }

    #[test]
    fn ingest_syncs_fts_rows_on_chunk_update() {
        let (_tempdir, _path) = db_path();
        let mut conn = connect(_path).expect("connect initializes database");
        let records = fixture_records();
        let mut updated_records = records.clone();
        for record in &mut updated_records {
            if let RetrievalRecord::Chunk(chunk) = record
                && chunk.chunk_id
                    == "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27:seerr-jellyfin-links:abc123"
            {
                chunk.body = "Updated externalHostname body for FTS refresh.".to_string();
                chunk.content_hash = "sha256:chunk-media-updated".to_string();
            }
        }
        let provider = HashEmbeddingProvider::default();

        ingest_records(&mut conn, &records, &provider, false).expect("fixture ingests");
        let summary = ingest_records(&mut conn, &updated_records, &provider, false)
            .expect("updated fixture ingests");

        let fts_body: String = conn
            .query_row(
                "SELECT body FROM chunks_fts WHERE chunk_id = ?",
                ["c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27:seerr-jellyfin-links:abc123"],
                |row| row.get(0),
            )
            .expect("fts row reads");
        assert_eq!(summary.chunks_upserted, 1);
        assert_eq!(summary.embeddings_computed, 1);
        assert_eq!(fts_body, "Updated externalHostname body for FTS refresh.");
    }

    #[test]
    fn ingest_populates_links_from_chunk_outgoing_ids() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let records = fixture_records()
            .into_iter()
            .filter(|record| !matches!(record, RetrievalRecord::Link(_)))
            .collect::<Vec<_>>();
        let provider = HashEmbeddingProvider::default();

        let summary =
            ingest_records(&mut conn, &records, &provider, false).expect("fixture ingests");
        let current = status(&conn, &path).expect("status reads");

        assert_eq!(summary.links_seen, 0);
        assert_eq!(summary.links_upserted, 0);
        assert_eq!(current.links, 1);
    }

    #[test]
    fn ingest_records_with_progress_uses_configured_embedding_max_body_chars() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let records = vec![
            RetrievalRecord::Note(NoteRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                note_id: "note-1".to_string(),
                path: "note.org".to_string(),
                title: "Note".to_string(),
                aliases: Vec::new(),
                tags: Vec::new(),
                updated_at: 1,
                content_hash: "sha256:note".to_string(),
            }),
            RetrievalRecord::Chunk(ChunkRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                chunk_id: "chunk-1".to_string(),
                note_id: "note-1".to_string(),
                path: "note.org".to_string(),
                title: "Note".to_string(),
                aliases: Vec::new(),
                tags: Vec::new(),
                heading_path: Vec::new(),
                heading_level: 1,
                body: "abcdef".to_string(),
                start_line: 1,
                end_line: 1,
                outgoing_ids: Vec::new(),
                updated_at: 1,
                content_hash: "sha256:chunk".to_string(),
            }),
        ];
        let provider = RecordingEmbeddingProvider::default();

        ingest_records_with_progress(&mut conn, &records, &provider, false, 3, |_| {})
            .expect("records ingest");

        let texts = provider.texts.lock().expect("texts lock");
        assert_eq!(texts.len(), 1);
        assert!(texts[0].ends_with("abc"));
        assert!(!texts[0].contains("def"));
    }

    fn fixture_records() -> Vec<RetrievalRecord> {
        load_ndjson(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/retrieval-export.ndjson"),
        )
        .expect("fixture records load")
    }

    #[test]
    fn search_builds_python_compatible_fts_query() {
        assert_eq!(
            build_fts_query(r#""pkms task" agenda"#),
            r#""pkms task" OR "agenda""#
        );
        assert_eq!(
            build_fts_query("UrlBase externalHostname"),
            r#""UrlBase" OR "externalHostname""#
        );
        assert_eq!(build_fts_query("   "), "");
    }

    #[test]
    fn search_finds_exact_technical_terms() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = search(&conn, "externalHostname", 10).expect("search succeeds");

        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.title, "Media Library Migration to Jellyfin");
        assert_eq!(result.heading_path, vec!["Seerr", "Jellyfin links"]);
        assert_eq!(result.start_line, 40);
        assert_eq!(result.end_line, 58);
        assert!(result.text.contains("externalHostname"));
        assert!(result.scores.bm25 > 0.0);
        assert_eq!(result.scores.final_score, result.scores.bm25);
    }

    #[test]
    fn search_returns_pkms_task_command_for_phrase_and_token_query() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = search(&conn, r#""pkms task" agenda"#, 10).expect("search succeeds");

        assert!(!results.is_empty());
        assert_eq!(results[0].title, "PKMS Task Backend");
        assert!(
            results[0]
                .text
                .contains("pkms task agenda today source:all")
        );
    }

    #[test]
    fn search_uses_or_semantics_for_multiple_tokens() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = search(&conn, "UrlBase externalHostname", 10).expect("search succeeds");

        assert!(!results.is_empty());
        assert_eq!(results[0].title, "Media Library Migration to Jellyfin");
    }

    #[test]
    fn search_empty_query_returns_no_results() {
        let (_tempdir, path) = db_path();
        let conn = connect(path).expect("connect initializes database");

        let results = search(&conn, "   ", 10).expect("search succeeds");

        assert!(results.is_empty());
    }

    #[test]
    fn dense_search_returns_related_fixture_chunks_with_hash_provider() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results =
            dense_search(&conn, "today agenda inspect tasks", 10, &provider).expect("dense search");

        assert!(!results.is_empty());
        assert_eq!(results[0].title, "PKMS Task Backend");
        assert!(results[0].scores.dense > 0.0);
        assert_eq!(results[0].scores.final_score, results[0].scores.dense);
        assert_eq!(results[0].scores.bm25, 0.0);
        assert!(
            results[0]
                .text
                .contains("pkms task agenda today source:all")
        );
    }

    #[test]
    fn dense_search_filters_embedding_model_and_dimension() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let wrong_dimension_provider = HashEmbeddingProvider::with_dimension(8);
        let wrong_dimension_results = dense_search(
            &conn,
            "today agenda inspect tasks",
            10,
            &wrong_dimension_provider,
        )
        .expect("dense search with mismatched dimension");
        assert!(wrong_dimension_results.is_empty());

        conn.execute("UPDATE chunk_embeddings SET model_name = 'other-model'", [])
            .expect("embedding model updates");
        let wrong_model_results = dense_search(&conn, "today agenda inspect tasks", 10, &provider)
            .expect("dense search with mismatched model");
        assert!(wrong_model_results.is_empty());
    }

    #[test]
    fn dense_search_clamps_negative_scores_to_zero() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");
        let query = "today agenda inspect tasks";
        let query_vector = provider
            .embed(&[query.to_string()])
            .expect("query embeds")
            .pop()
            .expect("query vector exists");
        let negative_query_vector = query_vector.iter().map(|value| -*value).collect::<Vec<_>>();
        conn.execute(
            "UPDATE chunk_embeddings SET vector = ?",
            [pack_vector(&negative_query_vector)],
        )
        .expect("embedding vectors update");

        let results = dense_search(&conn, query, 10, &provider).expect("dense search");

        assert!(!results.is_empty());
        assert!(results.iter().all(|result| result.scores.dense == 0.0));
        assert!(
            results
                .iter()
                .all(|result| result.scores.final_score == 0.0)
        );
    }

    #[test]
    fn dense_search_handles_zero_query_vectors_without_panics() {
        let (_tempdir, path) = db_path();
        let mut conn = connect(&path).expect("connect initializes database");
        let ingest_provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &ingest_provider, false)
            .expect("fixture ingests");
        let zero_provider = StaticEmbeddingProvider {
            model_name: "hashing-v1".to_string(),
            dimension: 384,
            vector: vec![0.0; 384],
        };

        let results =
            dense_search(&conn, "non-empty query", 10, &zero_provider).expect("dense search");

        assert!(!results.is_empty());
        assert!(results.iter().all(|result| result.scores.dense == 0.0));
    }

    struct StaticEmbeddingProvider {
        model_name: String,
        dimension: usize,
        vector: Vec<f32>,
    }

    impl EmbeddingProvider for StaticEmbeddingProvider {
        fn model_name(&self) -> &str {
            &self.model_name
        }

        fn dimension(&self) -> usize {
            self.dimension
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(vec![self.vector.clone(); texts.len()])
        }
    }

    #[derive(Default)]
    struct RecordingEmbeddingProvider {
        texts: Mutex<Vec<String>>,
    }

    impl EmbeddingProvider for RecordingEmbeddingProvider {
        fn model_name(&self) -> &str {
            "recording"
        }

        fn dimension(&self) -> usize {
            1
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            self.texts
                .lock()
                .expect("texts lock")
                .extend(texts.iter().cloned());
            Ok(vec![vec![0.0]; texts.len()])
        }
    }
}
