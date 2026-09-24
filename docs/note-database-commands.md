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
pkms resolve --title "project" --include-tags active --path-prefix roam/projects
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

### Shared scope filters

`resolve`, `query`, `orphans`, and `suggest` accept the same optional scope
filters:

```bash
pkms query "distributed systems" --include-tags project,active
pkms query "distributed systems" --exclude-tags archived,private
pkms query "distributed systems" --path-prefix roam/projects
pkms query "distributed systems" --without-dailies
pkms query "distributed systems" --modified-since 2026-08-01
```

All `--include-tags` values must be present; any `--exclude-tags` value rejects
a note. Tag matching is exact. Relative path prefixes are resolved against
`db_root`. `--modified-since` accepts a UTC `YYYY-MM-DD` date or an RFC 3339
timestamp and checks current source-file modification times. `--with-dailies`
and `--without-dailies` are mutually exclusive. Commands continue to include
daily notes by default except `orphans`, which continues to exclude them.

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

Choose at most one stats mode: `--days`, `--hubs`, `--tags`, or `--todos`.
Combinations are rejected rather than resolved by precedence.

Bare `suggest` returns at most ten candidates. Use `--all` only when an
unbounded result is intentional.

### Find Unlinked Mentions

`mentions` finds phrases that name another note by title or alias but are not
links. org-roam calls these unlinked references.

```bash
pkms mentions <uuid-or-title>                      # names in this note
pkms mentions <uuid-or-title> --incoming           # other notes naming this one
pkms mentions <uuid-or-title> --output-format ndjson
pkms mentions - < draft.org                        # a draft not yet saved
```

Matching is case-insensitive and Unicode-aware. A match must be a whole word:
the characters on both sides must not be letters, digits, or `_`. When names
overlap, the longest one wins, so `Machine Learning` beats `Learning`. The
scanner skips text that is not prose:

- links and bare URLs
- `#+` keyword lines, comments, and drawers
- `SCHEDULED:`, `DEADLINE:`, and `CLOSED:` lines
- src, example, and export blocks
- inline `~code~` and `=verbatim=`
- timestamps
- heading keywords, priorities, and tags

A note never reports itself or headings in its own file. The default mode
leaves out these candidate notes:

- notes with names shorter than 3 characters
- daily notes, unless `--with-dailies` is set
- heading nodes

With `--incoming`, every other note is a source, daily notes included, and the
minimum name length does not apply.

`already_linked` is true when the scanned text already contains an `id:` link
to the mentioned note elsewhere. Treat every record as a link candidate, not
as a link to insert. Names are matched within one line, so a title wrapped
across lines is not found.

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

Write the note with body content from a file, or from stdin with `-`:

```bash
pkms new "My Note" --create --body draft.org
generate-draft | pkms new "My Note" --create --tags "idea" --body -
```

The body is appended verbatim after the generated `:PROPERTIES:`, `#+title`,
and `#+filetags` header, so it should not contain its own header. A missing
final newline is added. `--body` requires `--create` and cannot be combined
with `--heading`.

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
