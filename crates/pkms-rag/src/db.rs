use std::{fs, path::Path};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::{
    models::{SUPPORTED_SCHEMA_VERSION, StatusResponse},
    schema::SCHEMA_SQL,
};

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

pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)
        .context("failed to initialize RAG SQLite schema")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn db_path() -> (tempfile::TempDir, PathBuf) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("nested").join("rag.sqlite3");
        (tempdir, path)
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
}
