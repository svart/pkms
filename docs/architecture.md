# Architecture

`pkms` is a local-first CLI over org-roam style notes. A normal invocation
resolves configuration, scans files, computes one result, prints to stdout, and
exits. The design favors predictable command-line behavior, parseable output,
and preserving user note data over background state.

## Runtime Shape

1. `pkms` parses CLI arguments with `clap`.
2. Configuration resolves once from `--db`, then `PKMS_DB_ROOT`, then
   `~/.config/pkms.toml`.
3. `pkms` resolves process environment inputs and passes typed parameters into
   domain crates.
4. Commands load only the data they need: configuration only, a graph, a
   workspace, task providers, an optional RAG SQLite index, or a foreground HTTP
   server.
5. Output is rendered as text, JSON, or NDJSON through shared output helpers.
6. The process exits, except for explicit foreground servers: `pkms serve` when
   built with `web` and `pkms rag serve` when built with `rag`.

Do not add implicit caches, daemons, watchers, or hidden persistent derived
state. The RAG SQLite index is explicit derived local state owned by
`pkms rag` in `rag` builds; source org files remain authoritative.

## Workspace Crates

| Crate | Role | Detailed docs |
|-------|------|---------------|
| `pkms` | Umbrella binary crate: CLI parser, config mapping, dispatch, output, and cross-domain orchestration. | [crates/pkms.md](crates/pkms.md) |
| `pkms-org` | Org discovery, parsing, graph, workspace loading, task extraction from org, and raw org edit primitives. | [crates/pkms-org.md](crates/pkms-org.md) |
| `pkms-db` | Note database command logic: health checks, validation, search, graph navigation, creation, extraction, and repair. | [crates/pkms-db.md](crates/pkms-db.md) |
| `pkms-task` | Task domain logic: canonical IDs, filters, providers, mutations, Todoist integration, and typed requests into org editing. | [crates/pkms-task.md](crates/pkms-task.md) |
| `pkms-rag` | Local retrieval: org export, chunking, SQLite index, embeddings, search, retrieval, and RAG HTTP API. | [crates/pkms-rag.md](crates/pkms-rag.md) |
| `pkms-web` | Local web viewer: note rendering, static assets, routes, previews, and foreground HTTP serving. | [crates/pkms-web.md](crates/pkms-web.md) |

## Dependency Direction

The dependency direction is intentionally one-way:

- `pkms` may depend on all domain crates.
- `pkms-db`, `pkms-rag`, `pkms-task`, and `pkms-web` may depend on `pkms-org`.
- Domain crates must not depend on `pkms` or on each other unless the boundary
  check explicitly allows it.
- Domain crates should not read process environment variables; `pkms` should
  resolve environment-dependent values and pass them through typed config.

Run `scripts/check-crate-boundaries.sh` after changing crate manifests. The
check inspects each package's all-feature transitive dependency tree, so an
indirect path to a forbidden domain crate fails like a direct manifest edge.

## Command Flow

CLI wiring lives in the umbrella crate:

- `crates/pkms/src/cli.rs` and `crates/pkms/src/cli/task.rs` define arguments.
- `crates/pkms/src/runner.rs` dispatches parsed commands.
- `crates/pkms/src/command_context.rs` bundles shared resolved config and output
  access for command adapters.
- `crates/pkms/src/commands/` adapts CLI arguments to domain crates and renders
  output.

Reusable behavior belongs in domain crates:

- Org syntax, graph, workspace loading, and raw org edits go in `pkms-org`.
- Note database command behavior goes in `pkms-db`.
- Task source selection, filtering, IDs, and mutation planning go in
  `pkms-task`.
- RAG indexing, embedding, search, retrieval, and API behavior go in
  `pkms-rag`.
- Web note rendering and HTTP routes for `pkms serve` go in `pkms-web`.

## Data Flows

### Note Database Commands

Commands such as `check`, `validate`, `resolve`, `query`, `get`, `stats`,
`orphans`, `path`, `suggest`, `new`, `extract`, and `fix` are reached through
`pkms` command adapters and use `pkms-db` plus `pkms-org` graph/workspace data.
See [Note Database Commands](note-database-commands.md).

### Task Commands

`pkms task` command adapters live under `crates/pkms/src/commands/task/`.
They map CLI filters and modifiers into `pkms-task`, which collects provider
items, preserves canonical local task IDs, and sends typed org edit requests to
`pkms-org` for local writes. Todoist support is behind the `todoist` feature.
See [TODO and Agenda](todo-agenda.md) and [Task System Design](task-system.md).

### Web Viewer

`pkms serve` is compiled only with `--features web`. The command adapter lives
in `pkms`, while note rendering, route handling, static assets, KaTeX, Syntect,
and preview behavior live in `pkms-web`. The server is foreground-only and does
not create persistent derived state. See [Web Viewer](web.md).

### RAG Retrieval

`pkms rag` is compiled only with `--features rag`. It uses `pkms-rag` to export
notes through `pkms-org`, chunk them, embed chunks, persist a local SQLite
retrieval index, search with FTS, retrieve with BM25/dense/hybrid scoring, and
serve a foreground local HTTP API/UI. See [RAG Retrieval](rag.md).

## Output Contracts

User-facing commands should support `--output-format json` unless their command
contract explicitly documents a text-only exception, such as `pkms rag index`.
Stream-like commands should support `ndjson` when practical. NDJSON producers
emit one JSON object per line, usually with `uuid`; consumers read targets from
stdin via automatic pipe detection or `--from-stdin`.

See [JSON and NDJSON Output](json-output.md) and [Pipelining](pipelining.md).

## Feature Flags

| Feature | Default | Purpose |
|---------|---------|---------|
| `todoist` | off | Enables Todoist task reads and writes through `pkms-task`. |
| `web` | off | Enables `pkms serve` and the optional `pkms-web` dependency. |
| `ssh` | off | Enables remote SSH `file:` link checks in `pkms-db`. |
| `rag` | off | Enables `pkms rag` and the optional `pkms-rag` dependency. |

`pkms-rag` has a crate-level default `fastembed` feature. The umbrella `pkms`
crate enables `pkms-rag` only through the `rag` feature; RAG builds use
FastEmbed unless the dependency configuration changes. The FastEmbed dependency
is configured with rustls for Hugging Face model downloads and ONNX Runtime
binary downloads.
