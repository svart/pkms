# Crate: pkms-db

`crates/pkms-db` owns reusable note database command logic. The umbrella
`pkms` crate adapts CLI arguments and output formatting; this crate performs
the database-oriented work.

## Responsibilities

- Whole-database health checks and note validation.
- UUID/title/tag resolution and fuzzy query behavior.
- Note retrieval, link neighborhoods, heading display, and shortest paths.
- Statistics, hubs, tags, TODO summaries, orphans, and suggestions.
- Note creation planning and writing.
- Heading extraction into a new note.
- Broken UUID link repair and misplaced attachment repair.
- Local file, attachment, ID, self-link, overlink, and cross-link checks.
- Optional SSH `file:` link checks behind the `ssh` feature.

## Main Modules

| Module | Purpose |
|--------|---------|
| `commands/check/` | Check data collection, model, rendering, and tests. |
| `commands/validate.rs` | Focused validation for selected targets. |
| `commands/resolve.rs` | Fast UUID/title/tag resolution. |
| `commands/query.rs` | Fuzzy title, alias, ref, tag, and content search. |
| `commands/get.rs` | Note content and neighborhood retrieval. |
| `commands/stats.rs` | Database statistics, hubs, tags, and TODO counts. |
| `commands/orphans.rs` | Orphan note discovery. |
| `commands/path.rs` | Shortest path between notes. |
| `commands/suggest.rs` | Related-note scoring. |
| `commands/new.rs` | New note filename, UUID, and boilerplate generation. |
| `commands/extract.rs` | Heading subtree extraction. |
| `commands/fix.rs` | UUID and attachment repair flows. |
| `link_check/` | Local and optional SSH link-check implementations plus shared result models. |

## Invariants

- Command logic should preserve parseable stdout and let command adapters choose
  text, JSON, or NDJSON presentation.
- Dry-run commands such as `fix` and `extract` must stay dry-run unless
  `--apply` is present.
- SSH checks are explicit: they run only with `--remote-file-links` and a build
  that enables the `ssh` feature.
- Link checks must account for both note-level and heading-level IDs.

## Boundaries

`pkms-db` may depend on `pkms-org` and the leaf `pkms-tokens` crate. It must not
depend on the umbrella `pkms` crate, `pkms-rag`, `pkms-task`, or `pkms-web`.
Keep CLI parser details and output format selection in `pkms`.

## Related Docs

- [Note Database Commands](../note-database-commands.md)
- [Command Reference](../commands.md)
- [Notes Database Format](../database-format.md)
- [Maintenance Workflows](../workflows.md)
- [JSON and NDJSON Output](../json-output.md)
