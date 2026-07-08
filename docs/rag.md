# RAG Retrieval

`pkms rag` is available in builds made with `--features rag`. It provides local
retrieval over an org-roam notes database, builds a SQLite index from current
notes, can ingest retrieval NDJSON directly, stores sparse and dense retrieval
data locally, and exposes the same data through CLI commands, structured output
where supported, and a foreground local HTTP server.

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

Override the index path while using the configured `pkms` database root:

```bash
pkms rag index --rag-db .data/pkms-rag.sqlite3
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

Then open the printed URL. The process stays in the foreground. The `pkms` CLI
uses the resolved `db_root` as the default serve source, or uses `--notes-root`
or `--index-source` when one is passed, so `pkms rag serve` starts a background
rebuild on launch. Use `GET /index/status` or the UI to watch progress. In
builds with the `web` feature, result titles open the rendered note viewer using
the same routes as `pkms serve`; without that feature, result titles remain
plain text.

## Architecture

The RAG implementation lives in the `pkms-rag` crate and is wired into the
umbrella `pkms` binary through the `rag` command namespace.

The current data flow is:

1. `pkms rag index` exports org notes from the resolved `pkms` database root.
2. Org notes are exported through `pkms-org` parsing, including note metadata,
   headings, tags, links, aliases, and source locations.
3. Exported notes are chunked and written as retrieval records.
4. The indexer ingests records into SQLite tables for notes, chunks, links,
   SQLite FTS rows, and embeddings.
5. Search and retrieval read the SQLite index and return cited chunks with note
   titles, paths, heading paths, source line ranges, scores, and text snippets.

`pkms rag ingest` accepts retrieval NDJSON directly. `pkms rag index` exports
current org notes from the resolved database root, ingests those records, and
removes stale indexed rows for records no longer present in the database root.
Use `pkms rag index --force-rebuild` to remove the current SQLite index and
sidecar files before rebuilding from scratch. `pkms rag serve` starts a
foreground HTTP server and starts a background rebuild from the source selected
when the server was launched.

`pkms rag index` is text-only. It reports foreground rebuild progress to stderr
while keeping the final index summary on stdout, and rejects `--output-format`.

The default index path is `.data/pkms-rag.sqlite3`. Override it with `--rag-db`,
`PKMS_RAG_DB`, or `[rag].rag_db` in `~/.config/pkms.toml`. Relative `[rag]`
paths are resolved under `db_root`. The index is derived local state; source
notes remain the authority.

## Source Selection

`pkms rag index` exports org notes from the resolved `pkms` database root.

Use `pkms rag ingest` when you already have retrieval NDJSON and want to upsert
it into the selected RAG database:

```bash
pkms rag ingest retrieval-export.ndjson --rag-db .data/pkms-rag.sqlite3
```

`pkms rag serve` uses the resolved database root by default, and accepts
`--notes-root` or `--index-source` for one foreground server run.

Persistent RAG defaults live under `[rag]`:

```toml
[rag]
rag_db = ".data/pkms-rag.sqlite3"
embedding_model = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"
# fastembed_model_dir = "models/paraphrase-multilingual-MiniLM-L12-v2"
```

## Embeddings

FastEmbed is the default embedding provider. `pkms-rag` builds FastEmbed with
rustls for Hugging Face model downloads and ONNX Runtime binary downloads. It
downloads model files on first use and then runs from its dependency-managed
local cache.

Set `[rag].fastembed_model_dir` or `PKMS_RAG_FASTEMBED_MODEL_DIR` to load
FastEmbed model files from a local directory and skip Hugging Face downloads
during model initialization. The environment variable overrides the config
value when set. The directory must contain the files for the configured
`PKMS_RAG_EMBEDDING_MODEL` or `[rag].embedding_model`, including
`tokenizer.json`, `config.json`, `special_tokens_map.json`,
`tokenizer_config.json`, and the model file path FastEmbed expects for that
model, such as `onnx/model.onnx` for the default multilingual MiniLM model.

Use `PKMS_RAG_EMBEDDING_PROVIDER=hash` for deterministic local tests and
fixtures. The hash provider exposes `hashing-v1` embeddings with a fixed
dimension.

`pkms rag index` accepts index-run embedding controls:

```bash
pkms rag index --embedding-batch-size 128 --embedding-max-body-chars 8000
```

`--embedding-batch-size` controls the FastEmbed batch size. The default is 256.
`--embedding-max-body-chars` controls how much chunk body text is included in
embedding input. The default is 8000 characters.

Embedding-related environment variables:

- `PKMS_RAG_EMBEDDING_PROVIDER`
- `PKMS_RAG_EMBEDDING_MODEL` (overrides `[rag].embedding_model`)
- `PKMS_RAG_FASTEMBED_MODEL_DIR` (overrides `[rag].fastembed_model_dir`)

Use the same provider and compatible model when querying an index that was
built with dense embeddings. `bm25` mode can search without using dense scores,
but `hybrid` and `dense` use the active embedding provider for the query vector.

## Commands

```bash
pkms rag status
pkms rag ingest retrieval-export.ndjson
pkms rag index
pkms rag index --force-rebuild
pkms rag index --embedding-batch-size 128 --embedding-max-body-chars 8000
pkms rag search "externalHostname"
pkms rag retrieve "agenda inspect tasks" --limit 5 --mode hybrid
pkms rag serve --host 127.0.0.1 --port 7337
```

`pkms rag search` uses SQLite FTS. `pkms rag retrieve` supports `hybrid`,
`bm25`, and `dense` modes. Text output is concise; JSON returns the full
response object, and NDJSON emits one result per line for search and retrieval.
Each search/retrieve result includes `uuid`, the source note UUID, so RAG output
can feed pipeline consumers such as `get`, `validate`, and `task list`.

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
pkms rag retrieve "distributed mesh" --output-format ndjson | pkms task list --from-stdin
```

## HTTP API

`pkms rag serve` serves a local browser UI and HTTP API:

- `GET /health` returns service liveness.
- `GET /status` returns SQLite index counts and embedding model names.
- `GET /index/status` returns background index progress.
- `POST /index/start` starts a background rebuild from the source selected when
  the server was launched.
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
resolved database root or the one-run source passed to `pkms rag serve`.

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
- In restricted networks, use `[rag].fastembed_model_dir` or
  `PKMS_RAG_FASTEMBED_MODEL_DIR` with a locally downloaded model snapshot.
- In corporate TLS interception environments, install the corporate root CA in
  the OS trust store first:

  ```bash
  sudo cp corp-root-ca.crt /usr/local/share/ca-certificates/
  sudo update-ca-certificates
  ```

  Then verify normal tools trust the endpoint, for example
  `curl https://huggingface.co/`. FastEmbed currently reaches Hugging Face
  through dependency HTTP clients configured for rustls/webpki roots, so the OS
  trust store alone may still be insufficient for a corporate MITM root. If the
  download still fails, use `[rag].fastembed_model_dir` or
  `PKMS_RAG_FASTEMBED_MODEL_DIR` with a locally downloaded model snapshot.
- For deterministic local tests or environments where dense embeddings are not
  required, set `PKMS_RAG_EMBEDDING_PROVIDER=hash` for both indexing and
  retrieval.
- If `pkms rag serve` prints an address but retrieval returns no results, check
  `/index/status` and `pkms rag status --rag-db <path>` to confirm the rebuild
  completed and chunks were indexed.
- If two commands appear to use different indexes, pass the same `--rag-db`
  path explicitly, set `PKMS_RAG_DB`, or configure `[rag].rag_db`.
