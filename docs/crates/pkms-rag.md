# Crate: pkms-rag

`crates/pkms-rag` owns local retrieval over org-roam notes. It exports org
notes, chunks them, stores sparse and dense retrieval data in SQLite, performs
search and retrieval, and serves the RAG HTTP API/UI.

The umbrella `pkms` binary exposes this crate through `pkms rag` only when built
with `--features rag`.

## Responsibilities

- Export notes from `pkms-org` parsed data into retrieval records.
- Parse retrieval NDJSON for direct ingest.
- Chunk notes with stable content hashes and source line ranges.
- Maintain the SQLite schema and ingest records into notes, chunks, links, FTS,
  and embedding tables.
- Provide embedding providers: FastEmbed by default and deterministic hash
  embeddings for tests and fixtures.
- Build FastEmbed with rustls for Hugging Face model downloads and ONNX Runtime
  binary downloads.
- Search with SQLite FTS.
- Retrieve cited chunks with BM25, dense, or hybrid scoring.
- Track background index progress.
- Serve the local HTTP API and browser UI used by `pkms rag serve`.
- Expose note-viewer delegation hooks so the umbrella `pkms` crate can wire
  RAG UI result links to `pkms-web` without making `pkms-rag` depend on
  `pkms-web`.

## Main Modules

| Module | Purpose |
|--------|---------|
| `models.rs` | Request, response, record, score, and progress models. |
| `schema.rs` | SQLite schema versioning and table definitions. |
| `db.rs` | SQLite connection, ingest, status, FTS, and dense search. |
| `org_export.rs` | Export org notes into retrieval records through `pkms-org`. |
| `ndjson.rs` | Retrieval NDJSON parsing and loading. |
| `chunking.rs` | Chunk construction, content hashes, and token/source metadata. |
| `embeddings.rs` | Embedding provider trait, FastEmbed, hash provider, and env config. |
| `indexer.rs` | Synchronous and background rebuild orchestration. |
| `retrieve.rs` | BM25, dense, and hybrid retrieval. |
| `api.rs` | Axum HTTP API and foreground server. |
| `web.rs` | Embedded browser UI assets. |

## Invariants

- The SQLite RAG database is derived local state; org source files or supplied
  retrieval NDJSON remain authoritative.
- `pkms rag index` and `/index/start` rebuild from the configured source and
  remove stale indexed rows for records no longer present.
- Search and retrieval responses must cite note title, path, heading path,
  source line range, scores, and chunk text.
- The foreground RAG server may start a background rebuild, but it is not a
  daemon or watcher.

## Configuration Surface

- `--rag-db`, `PKMS_RAG_DB`, or `[rag].rag_db`: SQLite index path. Default:
  `.data/pkms-rag.sqlite3`.
- `--notes-root`, `PKMS_RAG_NOTES_ROOT`, or `[rag].notes_root`: org notes root
  to export.
- `--index-source`, `PKMS_RAG_INDEX_SOURCE`, or `[rag].index_source`: retrieval
  NDJSON source.
- `--host`/`--port` or `PKMS_RAG_HOST`/`PKMS_RAG_PORT`: HTTP bind settings for
  `pkms rag serve`.
- `PKMS_RAG_EMBEDDING_PROVIDER`: `fastembed` or `hash`.
- `PKMS_RAG_EMBEDDING_MODEL`: FastEmbed model name.
- `PKMS_RAG_EMBEDDING_BATCH_SIZE`: FastEmbed batch size.
- `PKMS_RAG_EMBEDDING_MAX_BODY_CHARS`: body text budget used for embedding
  text.
- `PKMS_RAG_FASTEMBED_MODEL_DIR`: local FastEmbed model files directory used
  instead of downloading from Hugging Face during model initialization.

## Boundaries

`pkms-rag` may depend on `pkms-org`. It must not depend on `pkms`,
`pkms-db`, `pkms-task`, or `pkms-web`. CLI command parsing and stdout rendering
belong in `pkms`.

## Related Docs

- [RAG Retrieval](../rag.md)
- [Architecture](../architecture.md)
- [Command Reference](../commands.md#rag-retrieval)
- [JSON and NDJSON Output](../json-output.md)
