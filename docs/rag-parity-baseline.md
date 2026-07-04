# RAG Parity Baseline

This checklist captures the current behavior of the Python reference service
before the Rust `pkms-rag` crate is introduced.

Final Rust/Python comparison and migration recommendation:
[`docs/rag-parity-review.md`](rag-parity-review.md).

Reference repository: `/home/svart/work/my-projects/pkms-rag-service`
Reference commit: `0b506c7`
Fixture copied from: `examples/retrieval-export.ndjson`
Rust fixture target: `crates/pkms-rag/tests/fixtures/retrieval-export.ndjson`

## Fixture Records

The fixture contains five schema version 1 NDJSON records:

- 2 `note` records.
- 2 `chunk` records.
- 1 explicit `link` record from `PKMS Task Backend` to
  `Media Library Migration to Jellyfin`.

Expected fixture IDs:

- Media note: `c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27`.
- Media chunk:
  `c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27:seerr-jellyfin-links:abc123`.
- Task note: `aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa`.
- Task chunk:
  `aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa:todoist-agenda:def456`.

## Ingest And Status

With `HashEmbeddingProvider` and an empty SQLite database, ingesting the fixture
must produce:

- `notes_seen = 2`, `notes_upserted = 2`.
- `chunks_seen = 2`, `chunks_upserted = 2`, `chunks_unchanged = 0`.
- `links_seen = 1`, `links_upserted = 1`.
- `embeddings_computed = 2`, `embeddings_skipped = 0`.

Status after ingest:

- `schema_version = 1`.
- `notes = 2`.
- `chunks = 2`.
- `links = 1`.
- `stale_chunks = 0`.
- `fts_rows = 2`.
- `embeddings = 2`.
- `embedding_models = ["hashing-v1"]`.

Re-ingesting the same fixture must leave notes and chunks unchanged and compute
zero new embeddings.

Full rebuild with only the first note and chunk records must delete the missing
task note, task chunk, link, FTS row, and embedding.

## Search Baseline

`search("externalHostname", 10)` returns exactly one result:

- Title: `Media Library Migration to Jellyfin`.
- Heading path: `["Seerr", "Jellyfin links"]`.
- Source lines: `40..58`.
- Text contains `externalHostname`.
- `scores.bm25 > 0`.

`search("\"pkms task\" agenda", 10)` returns `PKMS Task Backend` first and the
text contains `pkms task agenda today source:all`.

`search("UrlBase externalHostname", 10)` returns `Media Library Migration to
Jellyfin` first.

## Dense Search Baseline

With the hash embedding provider, `dense_search("today agenda inspect tasks",
10)` returns `PKMS Task Backend` first with a positive dense score.

The hash provider metadata is:

- `model_name = "hashing-v1"`.
- `dimension = 384`.

## Retrieve Baseline

`retrieve("externalHostname", 5, "hybrid")` returns
`Media Library Migration to Jellyfin` first:

- Note ID: `c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27`.
- Path: `ops/media-library.org`.
- Heading path: `["Seerr", "Jellyfin links"]`.
- Source lines: `40..58`.
- Text contains `externalHostname`.
- `scores.final > 0`.
- Reason includes `bm25`.

The same query in `bm25` mode includes `PKMS Task Backend` as a graph-expanded
candidate with:

- `scores.bm25 = 0`.
- `scores.dense = 0`.
- `scores.graph > 0`.
- `reason = "graph-expanded"`.

`retrieve("agenda inspect tasks", 5, "hybrid")` returns `PKMS Task Backend`
first with a positive final score.

## API Baseline

The Python API contracts to preserve:

- `GET /health` returns `{"status":"ok"}`.
- `GET /status` returns the status shape above.
- `POST /ingest` accepts `application/x-ndjson` and returns `IngestSummary`.
- `POST /search` with `{"query":"UrlBase externalHostname","limit":5}`
  returns `Media Library Migration to Jellyfin` first.
- `POST /retrieve` with
  `{"query":"agenda inspect tasks","limit":5,"mode":"hybrid"}` returns
  `PKMS Task Backend` first.
- `GET /index/status` returns `phase = "idle"` when no index source is
  configured.
- `GET /` serves a page containing `PKMS Search`.
- `GET /ui.js` contains a poll of `/index/status`.

## Org Export Baseline

The Python exporter test creates this note:

```org
#+title: Semantic Search Notes
#+filetags: :pkms:rag:
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Retrieval workflow
Use [[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][linked context]] for semantic PKMS search.
```

Expected exported records:

- One note record with note ID `bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb`.
- Note title `Semantic Search Notes`.
- Note tags `["pkms", "rag"]`.
- One chunk record with path `semantic-search.org`.
- Chunk heading path `["Retrieval workflow"]`.
- Chunk outgoing IDs `["cccccccc-cccc-4ccc-cccc-cccccccccccc"]`.

The Rust exporter should intentionally improve on the Python regex exporter by
using `pkms-org` parsing, but differences must be covered by explicit tests.
