# RAG Parity Review

Date: 2026-07-04

This review compares the Rust `pkms-rag` implementation in the `pkms` workspace
with the Python reference service at
`/home/svart/work/my-projects/pkms-rag-service`.

## Verification

Python reference checks:

```bash
cd /home/svart/work/my-projects/pkms-rag-service
.venv/bin/pytest -q
```

Result: 14 passed, with one existing Starlette `TestClient` deprecation warning.
The API tests need to run outside the restricted sandbox; the sandboxed run
hangs in `TestClient`, while the same tests pass normally outside it.

Rust checks:

```bash
cargo fmt --all -- --check
scripts/check-crate-boundaries.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

Result: all checks passed.

Focused Rust RAG checks:

```bash
cargo test -p pkms-rag api
cargo test -p pkms --all-features rag
```

Result: all focused API and umbrella CLI RAG tests passed.

Live server checks:

- `pkms rag serve --rag-db <tmp>/rag.sqlite3 --host 127.0.0.1 --port 0`
  returned `{"status":"ok"}` from `/health`.
- `pkms rag serve --notes-root <tmp>/notes --rag-db <tmp>/rag.sqlite3`
  auto-started indexing and `/status` reported one note, one chunk, and one
  embedding.

## Fixture Parity

The Rust and Python implementations were run against the same
`retrieval-export.ndjson` fixture with `PKMS_RAG_EMBEDDING_PROVIDER=hash`.

Matching ingest summary:

- `notes_seen = 2`, `notes_upserted = 2`.
- `chunks_seen = 2`, `chunks_upserted = 2`, `chunks_unchanged = 0`.
- `links_seen = 1`, `links_upserted = 1`.
- `embeddings_computed = 2`, `embeddings_skipped = 0`.

Matching status after ingest:

- `schema_version = 1`.
- `notes = 2`, `chunks = 2`, `links = 1`.
- `stale_chunks = 0`, `fts_rows = 2`, `embeddings = 2`.
- `embedding_models = ["hashing-v1"]`.

Matching search result for `UrlBase externalHostname`:

- Top title: `Media Library Migration to Jellyfin`.
- Chunk ID:
  `c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27:seerr-jellyfin-links:abc123`.
- Path and lines: `ops/media-library.org:40-58`.
- Heading path: `["Seerr", "Jellyfin links"]`.
- BM25/final score: `9.416333545709269e-7`.

Matching retrieve result for `agenda inspect tasks` in `hybrid` mode:

- Top title: `PKMS Task Backend`.
- Chunk ID: `aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa:todoist-agenda:def456`.
- Path and lines: `tasks/pkms-task.org:10-16`.
- Heading path: `["Todoist agenda"]`.
- Scores: `bm25 = 1.0`, `dense = 1.0`, `metadata = 0.6666666666666666`,
  `graph = 0.9`, `final = 0.9880000000000001`.
- Reason: `bm25+dense+metadata+links`.

Matching API contract coverage:

- Python `GET /health`, `GET /status`, `GET /index/status`, `POST /ingest`,
  `POST /search`, `POST /retrieve`, and `POST /index/start` tests pass.
- Rust API tests cover the same endpoints plus invalid NDJSON rejection and a
  real TCP server smoke test.
- Rust `pkms rag serve` now starts a rebuild on launch when `--notes-root`,
  `--index-source`, `PKMS_RAG_NOTES_ROOT`, or `PKMS_RAG_INDEX_SOURCE` is set,
  matching the Python service lifecycle.

## Intentional Differences

Rust uses `pkms-org` for notes-root export instead of the Python regex exporter.
This is intentional. The Rust tests cover the expected parity fixture plus
improvements for ignore patterns, aliases, file-stem title fallback, heading
IDs, heading tags, property drawer exclusion, outgoing IDs, and source block
preservation.

Rust exposes the user-facing command as `pkms rag ...` instead of preserving the
standalone `pkms-rag` executable name. This matches the workspace decision that
`pkms` is the umbrella binary.

Rust keeps API state in an explicit `AppState` passed to the Axum router instead
of Python module globals. The HTTP route names and response shapes are
preserved.

Rust presents the Python-compatible default embedding model name
`sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2` to users. The
Rust FastEmbed provider internally maps it to the model identifier required by
the Rust crate when needed.

The Python CLI exposes retrieve weight flags. The Rust HTTP API accepts weights
in `RetrieveRequest`; the Rust CLI currently uses default weights. Add CLI
weight flags only if an actual local workflow depends on them.

## Migration

Use `pkms rag` as the preferred path:

| Python command | Rust command |
| --- | --- |
| `pkms-rag status --db DB` | `pkms rag status --rag-db DB` |
| `pkms-rag ingest PATH --db DB` | `pkms rag ingest PATH --rag-db DB` |
| `pkms-rag search QUERY --db DB` | `pkms rag search QUERY --rag-db DB` |
| `pkms-rag retrieve QUERY --db DB --mode hybrid` | `pkms rag retrieve QUERY --rag-db DB --mode hybrid` |
| `pkms-rag serve --db DB --notes-root NOTES` | `pkms rag serve --rag-db DB --notes-root NOTES` |

The main environment variables carry over:

- `PKMS_RAG_DB`
- `PKMS_RAG_NOTES_ROOT`
- `PKMS_RAG_INDEX_SOURCE`
- `PKMS_RAG_HOST`
- `PKMS_RAG_PORT`
- `PKMS_RAG_EMBEDDING_PROVIDER`
- `PKMS_RAG_EMBEDDING_MODEL`
- `PKMS_RAG_EMBEDDING_BATCH_SIZE`
- `PKMS_RAG_EMBEDDING_MAX_BODY_CHARS`

## Decision

The Rust implementation is ready to be the preferred implementation.

Recommended decommission path:

1. Keep the Python repository as a read-only reference through one `pkms`
   release containing `pkms rag`.
2. Migrate local scripts and docs from `pkms-rag` to `pkms rag`.
3. Archive the Python repository after the release soak unless a Docker-only or
   standalone-binary user still depends on it.

Do not delete the Python repository immediately; it remains useful as a compact
reference for response contracts and fixture history.
