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
- Provide shared tag, path, daily-note, and source modification-time filtering
  for read-only discovery and retrieval commands.

## Main Modules

| Module | Purpose |
|--------|---------|
| `discovery.rs` | Walks the note database and finds candidate org files. |
| `parser.rs` | Parses org note metadata, headings, links, TODO data, and planning data. |
| `corpus.rs` | Loads parsed notes from discovered files. |
| `graph/` | Builds graph storage plus search, analytics, validation, and traversal over parsed notes. |
| `snapshot.rs` | Loads parsed content and graph indexes from one fresh scan. |
| `scope.rs` | Applies shared note-scope filters against current source paths and tags. |
| `domain.rs` | Shared domain identifiers such as note IDs and link targets. |
| `attachments.rs` | Org-attach path and target helpers. |
| `link_check.rs` | Local link target checking helpers. |
| `mentions.rs` | Unlinked-mention matching over org prose: title/alias names, skipped org syntax, line/column hits. |
| `org_edit.rs` | Raw org edit primitives. |
| `org_task_edit.rs` | Local task heading edit operations. |
| `org_task_mutation.rs` | Typed task mutation requests applied to org files. |
| `org_date.rs` | Org timestamp and date parsing helpers. |

## Invariants

- Note-level IDs and heading-level IDs both participate in UUID resolution,
  duplicate-ID validation, links, and neighborhoods.
- `Graph::load_from()` and `OrgSnapshot::load()` always perform a fresh scan;
  neither introduces persistent derived state.
- Scanning accepts `ScanConfig`, including the resolved TODO-state keywords;
  link resolution accepts `LinkResolutionContext`. Note-creation directories
  are not loading inputs.
- `~` expansion for file links uses the injected `home_dir`; this crate does
  not resolve the process environment itself.
- Task state policy, task projection, and canonical task IDs belong to
  `pkms-task`, which consumes parsed org headings through this crate's models.
- Graph construction, analytics, and traversal implementation modules are
  private; callers use `Graph` methods and the intentionally public search and
  validation types.
- Org editing helpers should preserve user content around the specific edit.
- Scope filters use exact tags and current source-file metadata; they do not
  introduce cached note state.
- Domain crates and command adapters should use these helpers instead of
  open-coded org string manipulation.

## Task Project Edit Primitives

`pkms-org` parses note-level and heading-level `PROJECT` properties independently
and exposes both through its parsed note and heading models. It does not apply
property inheritance or decide whether a task needs a heading override; that
policy belongs to `pkms-task`.

For task writes, the org backend provides two policy-free typed mechanisms:

- `OrgTaskInsertSpec.project` optionally serializes a `:PROJECT:` entry in the
  new task heading's property drawer. Planning metadata is written before the
  drawer, consistent with existing task mutations.
- `org_task_mutation::HeadingMod.project` accepts `Change::Set`,
  `Change::Clear`, or `Change::Unchanged` and applies that operation only to the
  target heading's property drawer.

Insertion writes the task and optional project property together in one file
operation. Mutation reports the direct heading-property change; after a clear,
the effective task project may still come from the note-level property when
`pkms-task` reloads and projects the task.

## Boundaries

`pkms-org` is the lowest-level domain crate. It must not depend on `pkms`,
`pkms-db`, `pkms-rag`, `pkms-task`, `pkms-web`, or `pkms-tokens`. Higher-level
crates adapt its parsed models and edit primitives to commands.

## Related Docs

- [Architecture](../architecture.md)
- [Notes Database Format](../database-format.md)
- [Task System Design](../task-system.md)
- [Development](../development.md)
