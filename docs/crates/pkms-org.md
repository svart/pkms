# Crate: pkms-org

`crates/pkms-org` owns org-roam file discovery, org parsing, immutable snapshot
construction, graph indexes and traversal, link targets, and raw typed org edit
primitives.

## Responsibilities

- Discover org files under the resolved database root while honoring ignore
  patterns.
- Parse note-level and heading-level `:ID:` properties.
- Parse titles, aliases, refs, filetags, links, headings, TODO states,
  priorities, planning dates, and task metadata.
- Build graph data where note IDs and heading IDs are both first-class nodes.
- Provide one-scan `OrgSnapshot` loading when commands need parsed files plus
  graph data.
- Provide raw org editing helpers for note creation, heading extraction, task
  insertion, task planning-line edits, task state edits, and task subtree moves.
- Provide attachment path helpers and local link checks used by database
  commands.

## Main Modules

| Module | Purpose |
|--------|---------|
| `discovery.rs` | Walks the note database and finds candidate org files. |
| `parser.rs` | Parses org note metadata, headings, links, TODO data, and planning data. |
| `corpus.rs` | Loads parsed notes from discovered files. |
| `graph/` | Builds search, analytics, validation, traversal, and task views over parsed notes. |
| `snapshot.rs` | Loads parsed content and graph indexes from one fresh scan. |
| `domain.rs` | Shared domain identifiers such as note IDs and link targets. |
| `attachments.rs` | Org-attach path and target helpers. |
| `link_check.rs` | Local link target checking helpers. |
| `org_edit.rs` | Raw org edit primitives. |
| `org_task_edit.rs` | Local task heading edit operations. |
| `org_task_mutation.rs` | Typed task mutation requests applied to org files. |
| `org_date.rs` | Org timestamp and date parsing helpers. |

## Invariants

- Note-level IDs and heading-level IDs both participate in UUID resolution,
  duplicate-ID validation, links, and neighborhoods.
- `Graph::load_from()` and `OrgSnapshot::load()` always perform a fresh scan;
  neither introduces persistent derived state.
- Scanning accepts `ScanConfig`; link resolution accepts
  `LinkResolutionContext`. Note-creation directories are not loading inputs.
- `~` expansion for file links uses the injected `home_dir`; this crate does
  not resolve the process environment itself.
- Task state policy, task projection, and canonical task IDs belong to
  `pkms-task`, which consumes parsed org headings through this crate's models.
- Org editing helpers should preserve user content around the specific edit.
- Domain crates and command adapters should use these helpers instead of
  open-coded org string manipulation.

## Boundaries

`pkms-org` is the lowest-level domain crate. It must not depend on `pkms`,
`pkms-db`, `pkms-rag`, `pkms-task`, `pkms-web`, or `pkms-tokens`. Higher-level
crates adapt its parsed models and edit primitives to commands.

## Related Docs

- [Architecture](../architecture.md)
- [Notes Database Format](../database-format.md)
- [Task System Design](../task-system.md)
- [Development](../development.md)
