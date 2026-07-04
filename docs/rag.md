# RAG Retrieval

`pkms rag` provides local retrieval over an org-roam notes database. It builds a
SQLite index from current notes or retrieval NDJSON, stores sparse and dense
retrieval data locally, and exposes the same data through CLI commands, JSON or
NDJSON output, and a foreground local HTTP server.

## Architecture

The RAG implementation lives in the `pkms-rag` crate and is wired into the
umbrella `pkms` binary through the `rag` command namespace.

The current data flow is:

1. `pkms rag index` resolves a source from `--notes-root`,
   `PKMS_RAG_NOTES_ROOT`, `--index-source`, `PKMS_RAG_INDEX_SOURCE`, or the
   configured `pkms` database root.
2. Org notes are exported through `pkms-org` parsing, including note metadata,
   headings, tags, links, aliases, and source locations.
3. Exported notes are chunked and written as retrieval records.
4. The indexer ingests records into SQLite tables for notes, chunks, links,
   SQLite FTS rows, and embeddings.
5. Search and retrieval read the SQLite index and return cited chunks with note
   titles, paths, heading paths, source line ranges, scores, and text snippets.

`pkms rag ingest` accepts retrieval NDJSON directly. `pkms rag index` rebuilds
from the selected source and removes stale indexed rows for records no longer in
the source. `pkms rag serve` starts a foreground HTTP server and starts a
background rebuild when a notes root or index source is configured.

The default index path is `.data/pkms-rag.sqlite3`. Override it with `--rag-db`
or `PKMS_RAG_DB`. The index is derived local state; source notes remain the
authority.

## Embeddings

FastEmbed is the default embedding provider. It downloads model files on first
use and then runs from its local cache. FastEmbed uses `./.fastembed_cache` by
default; set `FASTEMBED_CACHE_DIR` to move that cache, or set `HF_HOME` to use
the Hugging Face cache location. `HF_HOME` takes precedence.

Use `PKMS_RAG_EMBEDDING_PROVIDER=hash` for deterministic local tests and
fixtures. The hash provider exposes `hashing-v1` embeddings with a fixed
dimension.

Embedding-related environment variables:

- `PKMS_RAG_EMBEDDING_PROVIDER`
- `PKMS_RAG_EMBEDDING_MODEL`
- `PKMS_RAG_EMBEDDING_BATCH_SIZE`
- `PKMS_RAG_EMBEDDING_MAX_BODY_CHARS`

## Commands

```bash
pkms rag status
pkms rag ingest retrieval-export.ndjson
pkms rag index
pkms rag index --notes-root ~/org
pkms rag index --index-source retrieval-export.ndjson
pkms rag search "externalHostname"
pkms rag retrieve "agenda inspect tasks" --limit 5 --mode hybrid
pkms rag serve --host 127.0.0.1 --port 7337
```

`pkms rag search` uses SQLite FTS. `pkms rag retrieve` supports `hybrid`,
`bm25`, and `dense` modes. Text output is concise; JSON returns the full
response object, and NDJSON emits one result per line for search and retrieval.

## HTTP API

`pkms rag serve` starts a local browser UI and HTTP API:

- `GET /health` returns service liveness.
- `GET /status` returns SQLite index counts and embedding model names.
- `GET /index/status` returns background index progress.
- `POST /index/start` starts a background rebuild from the configured source.
- `POST /ingest` accepts retrieval NDJSON with `application/x-ndjson`.
- `POST /search` accepts `{"query":"...","limit":10}`.
- `POST /retrieve` accepts `{"query":"...","limit":10,"mode":"hybrid"}`.

The server is a foreground local process. It does not add a daemon, watcher, or
persistent service beyond the SQLite index selected by `--rag-db` or
`PKMS_RAG_DB`.
