# Command Reference

Run `pkms <command> --help` for the authoritative flag list. This page groups
the main commands and common options.

For task-oriented recipes, start with [Scenario Guide](scenarios.md). For
database maintenance workflows, see
[Note Database Commands](note-database-commands.md).

## Global Flags

| Flag | Description |
|------|-------------|
| `--db PATH` | Path to org-roam database root. Overrides config. |
| `--output-format FMT` | `text`, `json`, or `ndjson`. |

## Health

```bash
pkms check
pkms check --stats
pkms check --id-links
pkms check --file-links
pkms check --remote-file-links
pkms check --attachment-links
pkms check --filetags
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
pkms validate <target>
```

`check` scans the database and returns exit code 1 when issues are found.
`validate` checks one note or reads targets from NDJSON stdin with
`--from-stdin`.

`check --remote-file-links` is explicit network access for SSH `file:` links
and requires a binary built with `--features ssh`. It implies file-link output
but does not imply attachment checks. Supported targets use TRAMP-style SSH
syntax with absolute remote paths:

```org
[[file:/ssh:host:/absolute/path]]
[[file:/ssh:user@host:/absolute/path]]
[[file:/ssh:user@host#222:/absolute/path::needle]]
```

Without `--remote-file-links`, SSH `file:` targets are skipped instead of being
treated as local absolute paths. `validate` is local-only and also skips SSH
file targets.

SSH checks use strict `~/.ssh/known_hosts` verification and passwordless
public-key authentication only. Standard identity files and the SSH agent are
tried automatically. Password and keyboard-interactive prompts are not used.
Missing remote files are reported in `broken_file_links`; auth, host-key,
timeout, unsupported syntax, and other SSH/SFTP failures are reported in
`file_link_errors`.

## Lookup and Search

```bash
pkms resolve --uuid <uuid>
pkms resolve --title <title>
pkms resolve --tags "tag1,tag2"
pkms resolve --title <title> --todos
pkms query "search terms"
pkms query "search terms" --title
pkms query "search terms" --tags
pkms query "search terms" --content
pkms query "search terms" --todos
```

## Navigation

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --heading "Section title"
pkms get <target> --headings --no-content
pkms path <from> <to>
```

Targets may be UUIDs, note titles, or file paths unless a command says
otherwise.

When built with `--features web`:

```bash
pkms serve <target>
pkms serve <target> --port 0
```

See [Web Viewer](web.md) for build/run instructions and local file-serving
rules.

`serve` starts a foreground local HTTP server and renders the selected note as
HTML:

- Internal `id:` links navigate to `/?id=<uuid>`, so the browser address bar
  tracks the rendered note.
- Local `file:` and `attachment:` links are served only when the rendered note
  declares the exact link. `file:` targets must resolve under the database root;
  `attachment:` targets must resolve under the supported org-attach roots.
- Free-standing `http://` and `https://` URLs become clickable after org links
  are resolved.
- Org tables, tag chips, planning badges, Syntect source highlighting, and
  static KaTeX formula HTML are rendered server-side.
- The note header includes an "Open in Emacs" button using the same
  `emacsclient -n` editor path as task opening.
- Floating contents and backlink panels are open by default on wide viewports
  and collapsed on constrained viewports.
- Hovering over an internal note link opens a scrollable note preview after a
  short delay.

## Tags

```bash
pkms tags
pkms tags --output-format json
pkms tags suggest 11111111-1111-4111-8111-111111111111
pkms tags suggest 12 --limit 3 --neighbors 30
pkms tags suggest 12 --apply
```

`pkms tags` lists tags with their usage counts, sorted by count descending and
then tag name. A use is one direct filetag assignment on a note or one direct
tag assignment on a heading; inherited tags are not counted again. JSON wraps
entries in `tags`, while NDJSON emits one `{tag,count}` entry per line.

`pkms tags suggest` requires the `rag` feature and an indexed corpus. It accepts
only full note UUIDs and canonical local task IDs so target interpretation is
unambiguous.

## RAG Retrieval

See [RAG Retrieval](rag.md) for architecture, index storage, embedding provider
configuration, and HTTP API details.

When built with `--features rag`:

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

`pkms rag` builds and queries a local SQLite retrieval index. The RAG database
path resolves from `--rag-db`, `PKMS_RAG_DB`, `[rag].rag_db`, then the default
`.data/pkms-rag.sqlite3`.

`pkms rag index` rebuilds from the resolved `pkms` database root. Use
`pkms rag ingest retrieval-export.ndjson` to upsert retrieval NDJSON instead.

`pkms rag serve` uses the resolved database root by default, and accepts
`--notes-root` or `--index-source` for one foreground server run.

`pkms rag index --force-rebuild` removes the selected SQLite index and sidecar
files before rebuilding from scratch.

`pkms rag index --embedding-batch-size N` controls FastEmbed batch size for one
index run. `--embedding-max-body-chars N` controls how much chunk body text is
included in embedding input for one index run.

`pkms rag index` is text-only. It writes rebuild progress to stderr, prints the
final summary to stdout, and rejects `--output-format`.

`pkms rag retrieve` returns cited chunks with `hybrid`, `bm25`, or `dense`
scoring. Text output separates results with blank lines; JSON returns the full
response, and NDJSON emits one result per line. RAG search and retrieve results
include `uuid` for the source note so they can be piped to note-target consumers.
Each text result prints its source path and line range, score and match reason,
title, then snippet on separate lines.

When the RAG feature is enabled, `pkms tags suggest` uses dense similarity to
recommend tags that already exist in the indexed corpus. Targets are either a
full note UUID or a canonical local `<ID>` task ID.
The default is a five-tag preview over twenty similar chunks. `--apply` adds the
recommendations without removing current tags. Note recommendations use
filetags, while task recommendations use heading-specific tags and exclude
inherited filetags. Applying changes org
source but does not automatically rebuild the derived RAG index.

`pkms rag serve` starts a foreground local HTTP server with the browser UI and
HTTP API:

- `GET /health` returns service liveness.
- `GET /status` returns SQLite index counts and embedding model names.
- `GET /index/status` returns background index progress.
- `POST /index/start` starts a background rebuild from the server's startup
  source.
- `POST /ingest` accepts `application/x-ndjson`.
- `POST /search` accepts `{"query":"...","limit":10}`.
- `POST /retrieve` accepts `{"query":"...","limit":10,"mode":"hybrid"}`.

On startup, `pkms rag serve` starts a rebuild from the resolved `db_root` or the
one-run source passed with `--notes-root` or `--index-source`.
In builds with the `web` feature, result titles in the browser UI open notes
through the same rendered viewer routes as `pkms serve`; without that feature,
titles remain plain text.

FastEmbed is the default embedding provider. For deterministic local tests, set
`PKMS_RAG_EMBEDDING_PROVIDER=hash`. To avoid model downloads in restricted
networks, point `[rag].fastembed_model_dir` or
`PKMS_RAG_FASTEMBED_MODEL_DIR` at a local FastEmbed model directory. The
environment variable overrides the config value.

## Statistics and Discovery

```bash
pkms stats
pkms stats --days 30
pkms stats --hubs
pkms stats --hubs 20
pkms stats --tags
pkms stats --todos
pkms orphans
pkms orphans --with-dailies
pkms orphans --limit 20
```

## Creation and Repair

```bash
pkms new "My Note"
pkms new "My Note" --create
pkms new "My Note" --create --tags "tag1,tag2"
pkms new "My Note" --create --aliases "Alias1,Alias2"
pkms new "Existing Note" --create --heading "Heading"
pkms extract <heading-uuid>
pkms extract <heading-uuid> "New Note Title" --apply
pkms fix uuid <broken-uuid> <replacement-uuid>
pkms fix uuid <broken-uuid> <replacement-uuid> --apply
pkms fix attach
pkms fix attach --apply
pkms fix attach --apply --copy
```

`extract` is a dry run unless `--apply` is present. It accepts a heading-level
UUID, creates a new note whose primary `:ID:` is that heading UUID, and replaces
the old subtree with an `id:` link heading. The optional new title changes only
the new note `#+title`; the replacement link label uses the original heading
title.

`fix uuid` replaces broken `id:` link UUIDs and is a dry run unless `--apply`
is present. `fix attach` repairs missing heading-scoped `attachment:` targets by
finding exactly one matching file under supported `.attach` roots and moving it
to the org-attach path expected for that heading. It also dry-runs by default;
pass `--copy` with `--apply` to copy instead of move.

## Suggestions

```bash
pkms suggest <uuid>
pkms suggest <uuid> --limit 5
pkms suggest <uuid> --exclude-orphans
```

## Tasks

See [TODO and Agenda](todo-agenda.md) for the full task guide.

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda overdue
pkms task agenda upcoming
pkms task agenda upcoming --days 7
pkms task calendar
pkms task calendar --months 3
pkms task calendar -m -1
pkms task list --group state
pkms task list --from-stdin
pkms task list state:TODO tags:work,!blocked
pkms task list after:tom before:"2026-06-19 12:00"
pkms task list --columns +Project
pkms task agenda week type:SCHED project:Alpha
pkms task <id> show
pkms task <id> open
pkms task <id> state WAITING
pkms task <id> done
pkms task inbox
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent task"
pkms task <id> mod sch:2026-05-24
pkms task <id> mod dep:<parent-id>
pkms task <id> postpone
pkms task <id> postpone --to 2026-06-01
```

On ANSI-capable terminals, task text output renders inline `=code=`,
`~orange code~`, and mentions such as `@alice`. Piped text output keeps the
stored strings unchanged unless ANSI output is forced. Changed-task detail text
output starts with `Task: <task title>` before the change lines.
If a successful task mutation changes the canonical task ID assignment, pkms
prints a colored `WARN: Task IDs changed` line to stderr after the command
output. JSON/NDJSON stdout is unchanged and remains parseable.

See [TODO and Agenda](todo-agenda.md) for the full task guide, including task
IDs, filters, table columns, editor behavior, source selection,
state changes, and show output parent/child chains.

`pkms task inbox` and default `pkms task add` use the PKMS inbox note configured
as `[tasks].inbox`. The value can be a note title, UUID, absolute path, or path
relative to `db_root`. Set it to `daily` to use today's daily note and place
new tasks under `* Inbox`. New daily notes are created under `daily_notes_dir`,
or `new_notes_dir` when `daily_notes_dir` is unset.
Use `dep:<task-id>` or `depend:<task-id>` to add a PKMS task as the final child
heading of an existing PKMS task subtree.

Task schedule and deadline dates accept unambiguous prefixes of `today`,
`tomorrow`, or weekday names; weekdays resolve to the next upcoming matching
weekday. They also accept `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, or `HH:MM`; a time
without a date uses today.

## Configuration

```bash
pkms info
pkms init-config
pkms init-config --db ~/Documents/org
```
