# RAG Retrieval

`pkms rag` is available in builds made with `--features rag`. It provides local
retrieval over an org-roam notes database, builds a SQLite index from current
notes or retrieval NDJSON, stores sparse and dense retrieval data locally, and
exposes the same data through CLI commands, JSON or NDJSON output, and a
foreground local HTTP server.

For implementation boundaries, see the [pkms-rag crate docs](crates/pkms-rag.md).

## Quick Start

Install or run with the feature enabled:

```bash
cargo install --path . --features rag
cargo run --features rag -- --db ~/Documents/org rag status
```

Use the configured `pkms` database root:

```bash
pkms init-config --db ~/Documents/org
pkms rag index
pkms rag status
pkms rag retrieve "agenda inspect tasks" --limit 5 --mode hybrid
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
pkms rag serve --host 127.0.0.1 --port 7337
```

Then open the printed URL. The process stays in the foreground. When a notes
root or index source is configured, `pkms rag serve` starts a background rebuild
on launch; use `GET /index/status` or the UI to watch progress. In builds with
the `web` feature, result titles open the rendered note viewer using the same
routes as `pkms serve`; without that feature, result titles remain plain text.

## Architecture

The RAG implementation lives in the `pkms-rag` crate and is wired into the
umbrella `pkms` binary through the `rag` command namespace.

The current data flow is:

1. `pkms rag index` resolves a source from CLI/env source flags,
   `[rag].notes_root`, `[rag].index_source`, or the configured `pkms` database
   root.
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

With text output, `pkms rag index` reports foreground rebuild progress to
stderr while keeping the final index summary on stdout. JSON and NDJSON output
remain structured stdout only and emit the final progress object.

The default index path is `.data/pkms-rag.sqlite3`. Override it with `--rag-db`,
`PKMS_RAG_DB`, or `[rag].rag_db` in `~/.config/pkms.toml`. Relative `[rag]`
paths are resolved under `db_root`. The index is derived local state; source
notes remain the authority.

## Source Selection

`pkms rag index` and `pkms rag serve` select an input source in this order:

1. `--notes-root` or `PKMS_RAG_NOTES_ROOT`.
2. `--index-source` or `PKMS_RAG_INDEX_SOURCE`.
3. `[rag].notes_root` from `~/.config/pkms.toml`.
4. `[rag].index_source` from `~/.config/pkms.toml`.
5. The resolved `pkms` database root.

Examples:

```bash
pkms rag index --notes-root ~/Documents/org
pkms rag index --index-source retrieval-export.ndjson
PKMS_RAG_NOTES_ROOT=~/Documents/org pkms rag index
PKMS_RAG_INDEX_SOURCE=retrieval-export.ndjson pkms rag index
```

Persistent RAG defaults live under `[rag]`:

```toml
[rag]
rag_db = ".data/pkms-rag.sqlite3"
notes_root = "/home/user/Documents/org"
# index_source = "retrieval-export.ndjson"
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
- `FASTEMBED_CACHE_DIR`
- `HF_HOME`
- `HF_ENDPOINT`

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

In the browser UI, each result title links to `/?id=<note-id>` so the note opens
with the same rendered viewer used by `pkms serve` when the binary includes the
`web` feature. Without that feature, the UI leaves titles unlinked and direct
`/?id=<note-id>` requests fall back to the search UI.

The server is a foreground local process. It does not add a daemon, watcher, or
persistent service beyond the SQLite index selected by `--rag-db` or
`PKMS_RAG_DB` or `[rag].rag_db`. On startup it begins a rebuild from the
resolved source.

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

- If indexing fails during embedding setup, `pkms` prints the full error chain
  from the embedding provider. FastEmbed downloads the model on first use, so
  failures such as certificate verification errors, proxy connection errors,
  DNS errors, or HTTP status errors usually mean the process cannot reach the
  Hugging Face model repository from the current network.
- In restricted networks, configure `HF_ENDPOINT` for an internal Hugging Face
  mirror or pre-populate the FastEmbed cache in `HF_HOME` or
  `FASTEMBED_CACHE_DIR` from a machine that can download the model. `HF_HOME`
  takes precedence over `FASTEMBED_CACHE_DIR`.
- For deterministic local tests or environments where dense embeddings are not
  required, set `PKMS_RAG_EMBEDDING_PROVIDER=hash` for both indexing and
  retrieval.
- If `pkms rag serve` prints an address but retrieval returns no results, check
  `/index/status` and `pkms rag status --rag-db <path>` to confirm the rebuild
  completed and chunks were indexed.
- If two commands appear to use different indexes, pass the same `--rag-db`
  path explicitly, set `PKMS_RAG_DB`, or configure `[rag].rag_db`.
