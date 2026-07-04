# RAG Retrieval

`pkms rag` provides local retrieval over an org-roam notes database. It builds a
SQLite index from current notes or retrieval NDJSON, stores sparse and dense
retrieval data locally, and exposes the same data through CLI commands, JSON or
NDJSON output, and a foreground local HTTP server.

For implementation boundaries, see the [pkms-rag crate docs](crates/pkms-rag.md).

## Quick Start

Use the configured `pkms` database root:

```bash
pkms init-config --db ~/Documents/org
pkms rag index --rag-db .data/pkms-rag.sqlite3
pkms rag status --rag-db .data/pkms-rag.sqlite3
pkms rag retrieve "agenda inspect tasks" --limit 5 --mode hybrid \
  --rag-db .data/pkms-rag.sqlite3
```

Or pass the notes root directly:

```bash
pkms rag index --notes-root ~/Documents/org --rag-db .data/pkms-rag.sqlite3
pkms rag search "externalHostname" --limit 10 --rag-db .data/pkms-rag.sqlite3
```

FastEmbed is the default embedding provider. It may download model files on
first use. For deterministic local tests or environments where model downloads
are not desired, use the hash provider consistently for both indexing and
retrieval:

```bash
export PKMS_RAG_EMBEDDING_PROVIDER=hash
pkms rag index --rag-db /tmp/pkms-rag.sqlite3
pkms rag retrieve "agenda inspect tasks" --rag-db /tmp/pkms-rag.sqlite3
```

Serve the local browser UI and HTTP API:

```bash
pkms rag serve --rag-db .data/pkms-rag.sqlite3 --host 127.0.0.1 --port 7337
```

Then open the printed URL. The process stays in the foreground. When a notes
root or index source is configured, `pkms rag serve` starts a background rebuild
on launch; use `GET /index/status` or the UI to watch progress.

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

## Source Selection

`pkms rag index` selects an input source in this order:

1. `--notes-root` or `PKMS_RAG_NOTES_ROOT`.
2. `--index-source` or `PKMS_RAG_INDEX_SOURCE`.
3. The resolved `pkms` database root.

Examples:

```bash
pkms rag index --notes-root ~/Documents/org
pkms rag index --index-source retrieval-export.ndjson
PKMS_RAG_NOTES_ROOT=~/Documents/org pkms rag index
PKMS_RAG_INDEX_SOURCE=retrieval-export.ndjson pkms rag index
```

`pkms rag ingest` reads retrieval NDJSON and upserts it into the selected RAG
database without selecting a notes root:

```bash
pkms rag ingest retrieval-export.ndjson --rag-db .data/pkms-rag.sqlite3
```

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

Use the same provider and compatible model when querying an index that was
built with dense embeddings. `bm25` mode can search without using dense scores,
but `hybrid` and `dense` use the active embedding provider for the query vector.

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

Use a token budget when passing retrieval output to an LLM context:

```bash
pkms rag retrieve "task system canonical IDs" \
  --limit 12 \
  --max-token-budget 2000 \
  --output-format json
```

Use NDJSON when another tool should consume individual results:

```bash
pkms rag search "org attach" --output-format ndjson
pkms rag retrieve "RAG HTTP API" --limit 5 --output-format ndjson
```

## HTTP API

`pkms rag serve` serves a local browser UI and HTTP API:

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

Example API calls:

```bash
curl http://127.0.0.1:7337/status
curl -X POST http://127.0.0.1:7337/search \
  -H 'content-type: application/json' \
  -d '{"query":"externalHostname","limit":5}'
curl -X POST http://127.0.0.1:7337/retrieve \
  -H 'content-type: application/json' \
  -d '{"query":"agenda inspect tasks","limit":5,"mode":"hybrid"}'
```

## Troubleshooting

- If indexing fails during embedding setup, first check whether the FastEmbed
  model can be downloaded or use `PKMS_RAG_EMBEDDING_PROVIDER=hash` for a local
  deterministic run.
- If `pkms rag serve` prints an address but retrieval returns no results, check
  `/index/status` and `pkms rag status --rag-db <path>` to confirm the rebuild
  completed and chunks were indexed.
- If two commands appear to use different indexes, pass the same `--rag-db`
  path explicitly or set `PKMS_RAG_DB`.
