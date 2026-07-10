# ADR 0001: Maintainable crate boundaries

Status: Accepted on 2026-07-10.

## Context

The workspace already had a one-way dependency shape, but several ownership
details contradicted it. Task policy and projection leaked into `pkms-org` and
`pkms-db`; domain code could launch editors; graph storage fields and SQLite
connections were public; loading configuration mixed scan, link, and creation
paths; process environment reads were distributed; and tokenization made every
org consumer depend transitively on `tiktoken-rs`.

These leaks made a local change span unrelated crates and allowed invalid
orchestration states to compile. They also hid dead code behind broad public
facades.

## Decision

- `pkms` owns process concerns: CLI parsing, immutable `RuntimeInputs`, config
  precedence, rendering, exit codes, editor execution, and cross-domain wiring.
  Its library facade exposes only `run()`.
- `pkms-org` owns syntax, discovery, focused scan/link inputs, one-scan
  `OrgSnapshot`, graph queries, link resolution, and raw typed org edits. Graph
  storage and construction/traversal modules are private.
- `pkms-task` owns task state policy, org-heading projection, canonical IDs,
  filters, providers, show behavior, and mutations. Parsed filter internals are
  private behind read-only accessors.
- `pkms-db` owns note-database use cases and no task-only or process policy.
- `pkms-rag` owns retrieval use cases through `RagIndex`. SQLite connections
  and queries are private, index phases/steps are enums, synchronous failures
  are returned, and background status is an adapter concern.
- `pkms-web` owns viewer behavior behind a narrow root facade and a conventional
  private `serve` module tree. Editor execution remains an injected callback.
- `pkms-tokens` is a leaf utility for encoding, counting, and truncation only.
  Callers retain token-budget policy.
- Normal commands remain stateless fresh scans. Only explicit foreground web
  servers and the explicit derived RAG index are exceptions described in the
  architecture guide.

The transitive boundary check is the executable source of truth for forbidden
crate edges, including the token leaf.

## Consequences

Changes to task meaning should normally stay in `pkms-task` plus a thin CLI
adapter. Org graph representation and RAG storage can evolve without editing
all consumers. Configuration and tests can inject deterministic runtime inputs
without mutating global environment state. Crate-root APIs are smaller, so the
compiler can identify unused implementation helpers.

The decision introduces more focused config types and explicit adapters. This
is intentional: the extra construction code is confined to `pkms`, where
cross-domain and process-level knowledge belongs.
