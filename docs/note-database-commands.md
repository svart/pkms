# Note Database Commands

This page shows practical scenarios for the note database command family:
`check`, `validate`, `resolve`, `query`, `get`, `path`, `stats`, `orphans`,
`suggest`, `new`, `extract`, and `fix`.

For the complete flag list, use [Command Reference](commands.md) or
`pkms <command> --help`.

## Before You Start

Point `pkms` at a database with one of these:

```bash
pkms --db ~/Documents/org info
PKMS_DB_ROOT=~/Documents/org pkms info
pkms init-config --db ~/Documents/org
```

`--db` wins over `PKMS_DB_ROOT`, which wins over `db_root` in
`~/.config/pkms.toml`.

## Check Database Health

Use `check` for whole-database health and `validate` for focused checks:

```bash
pkms validate <changed-note>
pkms check
pkms check --stats
pkms check --id-links
pkms check --file-links
pkms check --attachment-links
pkms check --filetags
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
```

`check` returns a nonzero exit code when it finds unhealthy database state.
`validate` accepts a UUID, path, or title, and can read NDJSON targets from
stdin:

```bash
pkms query "project alpha" --output-format ndjson | pkms validate --from-stdin
```

Remote SSH `file:` link checks are explicit and require `--features ssh`:

```bash
cargo run --features ssh -- check --remote-file-links
```

Without `--remote-file-links`, SSH targets are skipped instead of being treated
as local absolute paths.

## Find Notes

Use `resolve` when you know a UUID fragment, title fragment, alias, or filetag:

```bash
pkms resolve --uuid <uuid-fragment>
pkms resolve --title "graph"
pkms resolve --tags "project,active"
pkms resolve --title "project" --todos
pkms resolve --tags "project" --fields uuid,title,path,tags
```

Use `query` when you want fuzzy search across titles, aliases, refs, tags, or
content:

```bash
pkms query "distributed systems" --limit 10
pkms query "agenda" --title
pkms query "rust" --content
pkms query "project" --todos
pkms query "project" --max-matches-per-note 5
```

Content matches are capped at three per note by default in text and structured
output. The result's `content_matches_total` field preserves the uncapped count;
use `--all-matches` when the full list is required.

## Inspect and Navigate

Use `get` to inspect a note and its immediate graph context:

```bash
pkms get <uuid-or-title>
pkms get <uuid-or-title> --links
pkms get <uuid-or-title> --headings --no-content
pkms get <uuid-or-title> --heading "Implementation Notes"
```

Use `path` for shortest graph paths:

```bash
pkms path "Source Note" "Target Note"
```

Use `stats`, `orphans`, and `suggest` to inspect structure and discover missing
connections:

```bash
pkms stats
pkms stats --days 30
pkms stats --hubs 20
pkms stats --tags
pkms stats --todos
pkms orphans --limit 20
pkms suggest <uuid> --limit 10 --exclude-orphans
```

Bare `suggest` returns at most ten candidates. Use `--all` only when an
unbounded result is intentional.

TODO-aware discovery, heading inspection, and statistics use the configured
`[agenda].open_todo_states` and `[agenda].closed_todo_states` lists.

## Create and Restructure Notes

Preview a new note path and UUID:

```bash
pkms new "My Note"
```

Write the boilerplate file:

```bash
pkms new "My Note" --create --tags "project,idea" --aliases "Alias One,Alias Two"
```

Generate a heading ID in an existing note:

```bash
pkms new "Existing Note" --create --heading "Section Title"
```

Extract a heading subtree into a new note:

```bash
pkms extract <heading-uuid>
pkms extract <heading-uuid> "New Note Title" --apply
```

`extract` is a dry run unless `--apply` is present. It creates a new note whose
primary `:ID:` is the heading UUID and replaces the old subtree with an `id:`
link heading.

## Repair Links and Attachments

Replace a broken UUID link target:

```bash
pkms fix uuid <broken-uuid> <replacement-uuid>
pkms fix uuid <broken-uuid> <replacement-uuid> --apply
```

Repair misplaced heading-scoped attachments:

```bash
pkms fix attach
pkms fix attach --apply
pkms fix attach --apply --copy
```

Both `fix` subcommands dry-run by default. Inspect proposed changes before
using `--apply`.

## Use Pipelines

NDJSON producers and consumers let agents scope batch work without temporary
files:

```bash
pkms query "topic" --output-format ndjson | pkms get --links --from-stdin
pkms resolve --tags "project" --output-format ndjson | pkms validate --from-stdin
pkms resolve --tags "project" --output-format ndjson | pkms task list --from-stdin
```

See [Pipelining](pipelining.md) and [JSON and NDJSON Output](json-output.md)
for output contracts.

## Related Docs

- [Command Reference](commands.md)
- [Notes Database Format](database-format.md)
- [Maintenance Workflows](workflows.md)
- [pkms-db crate](crates/pkms-db.md)
- [pkms-org crate](crates/pkms-org.md)
