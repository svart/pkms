# pkms Commands Overview

Use `pkms <command> --help` for exact current flags. This page is a compact map
for choosing commands.

Optional command surfaces depend on build features: `pkms serve` requires
`web`, `pkms rag` requires `rag`, and SSH remote file-link checks require `ssh`.

## Health

```bash
pkms check
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

`check --remote-file-links` requires an `ssh` feature build and explicitly
checks SSH `file:` links. Without that flag, SSH file targets are skipped.

## Lookup and Search

```bash
pkms resolve --title <term>
pkms resolve --uuid <uuid-fragment>
pkms resolve --tags "tag1,tag2"
pkms resolve --title <term> --todos
pkms query "terms"
pkms query "terms" --title
pkms query "terms" --tags
pkms query "terms" --content
pkms query "terms" --todos
pkms query "terms" --max-matches-per-note 5
pkms query "terms" --all-matches
```

Query results include at most three content matches per note by default and
report the uncapped count in `content_matches_total`.

`resolve`, `query`, `orphans`, `suggest`, `rag search`, `rag retrieve`, and
`tags suggest` share exact tag inclusion/exclusion, path-prefix, daily-mode, and
source modification-time scope flags. These filters run before command limits.

## Inspect and Navigate

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --heading "Section title"
pkms get <target> --headings --no-content
pkms path <from> <to>
```

`serve` exists only in builds made with the `web` feature:

```bash
pkms serve <target>
```

It starts a foreground local web viewer. Internal links navigate with
`/?id=<uuid>`; local file and attachment links are served as static assets when
they resolve to allowed local paths. The initial target may be a UUID, title, or
file path. The viewer shows collapsible floating
contents and backlinks panels by default, plus hover previews for internal note
links.

`pkms rag` exists only in builds made with the `rag` feature. Use it for local
retrieval indexing, search, cited retrieval, and the RAG HTTP API/UI.
Use `rag status --output-format json` to compare current source IDs with indexed
IDs and check the last successful full rebuild.
RAG search/retrieve JSON and NDJSON results include `uuid` for the source note,
so they can feed note-target pipeline consumers.
Use `[rag].fastembed_model_dir` or `PKMS_RAG_FASTEMBED_MODEL_DIR` when FastEmbed
should load model files from a local directory instead of downloading them.

Graph-based note suggestions return at most ten candidates per target by
default. Use `pkms suggest <uuid> --limit N` for another positive bound or
`pkms suggest <uuid> --all` for every candidate.

List direct note and heading tag assignments, sorted by descending usage count:

```bash
pkms tags
```

Recommend existing tags from semantically similar indexed content. Suggestions
require the `rag` feature and preview unless `--apply` is passed:

```bash
pkms tags suggest <full-note-uuid>
pkms tags suggest <canonical-id>
pkms tags suggest 12 --limit 5 --neighbors 20 --apply
```

Note recommendations use note filetags. Local task recommendations use
heading-only tags and exclude inherited filetags. Applying adds tags without
removing current tags.

## Create and Repair

```bash
pkms new "Title"
pkms new "Title" --create --tags "tag1,tag2"
pkms new "Existing Note" --create --heading "Heading"
pkms extract <heading-full-uuid>
pkms extract <heading-full-uuid> "New Note Title" --apply
pkms fix uuid <broken-full-uuid> <replacement-full-uuid>
pkms fix uuid <broken-full-uuid> <replacement-full-uuid> --apply
pkms fix attach
pkms fix attach --apply
pkms fix attach --apply --copy
```

`extract` requires a heading UUID. It is a dry run unless `--apply` is present.
The optional title changes only the new note `#+title`; the old subtree is
replaced by a heading link whose label is the original heading title.

`fix uuid` requires full UUIDs for both arguments. `fix attach` repairs
heading-scoped `attachment:` files when exactly one matching file exists under
the supported `.attach` roots, without rewriting Org link text.

## Tasks

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda overdue
pkms task agenda upcoming
pkms task agenda upcoming --days 7
pkms task list state:TODO tags:work,!blocked
pkms task agenda week type:SCHED project:Alpha
pkms task <canonical-id> show
pkms task <canonical-id> open
pkms task <canonical-id> state WAITING
pkms task <canonical-id> done
pkms task inbox
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent task"
pkms task <canonical-id> postpone
pkms task <canonical-id> postpone --to 2026-06-01
pkms task <canonical-id> mod sch:2026-05-24
pkms task <canonical-id> mod state:WAITING
pkms task <canonical-id> mod dl:2026-05-30
pkms task <canonical-id> mod dep:<parent-id>
```

## Statistics

```bash
pkms stats
pkms stats --hubs
pkms stats --tags
pkms stats --todos
pkms orphans
```

Stats mode selectors (`--days`, `--hubs`, `--tags`, and `--todos`) are mutually
exclusive.
