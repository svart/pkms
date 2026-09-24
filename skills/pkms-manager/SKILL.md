---
name: pkms-manager
description: Manage and navigate an org-roam PKMS database with the pkms CLI. Use when the user asks to search notes, inspect note content or links, validate database health, fix broken links, create notes, work with TODO/agenda tasks, build AI context, use pkms pipelines, or operate on an org-roam knowledge base. Prefer current `pkms <command> --help` for exact flags and consult bundled references only as needed.
---

# pkms-manager

Use this skill to operate the `pkms` CLI against an org-roam notes database.
Keep work grounded in live command behavior: run `pkms <command> --help` when
exact flags matter.

## Start Here

Before any database workflow, verify the active configuration:

```bash
pkms info
```

If `db_root` is wrong, pass `--db /actual/path` to commands. For reported tool
bugs against the current database, build the repository binary and reproduce
with `target/debug/pkms` before analysis.

For hard-to-explain CLI behavior, enable diagnostics on stderr while keeping
stdout parseable:

```bash
PKMS_LOG=debug pkms --output-format json <command>
```

Prefer structured output when an agent must parse results:

```bash
pkms --output-format json <command>
pkms <producer> --output-format ndjson | pkms <consumer> --from-stdin
```

The `pkms rag index` command is text-only and rejects `--output-format`. Run
`pkms rag status --output-format json` after indexing when automation needs
structured RAG index state.

## Reference Map

Load only the file needed for the task:

- Command syntax overview: `references/commands.md`
- Health and validation: `references/check.md`, `references/validate.md`
- Lookup and search: `references/resolve.md`, `references/query.md`
- Note inspection and graph navigation: `references/get.md`, `references/path.md`
- Note creation and repair: `references/new.md`, `references/fix.md`
- Suggestions and orphan linking: `references/suggest.md`, `references/orphans.md`
- Link candidates and unlinked references: `references/mentions.md`
- TODO and agenda tasks: `references/task.md`
- Pipelines: `references/pipelining.md`
- RAG retrieval and tag recommendations: `references/rag.md`
- Configuration: `references/info.md`, `references/init-config.md`

JSON schemas for maintained structured outputs live in `schemas/`.

## Core Workflows

### Discover Notes

1. Use `pkms resolve --title <term>` for fast title/alias lookup.
2. Use `pkms query "<terms>"` for broad title/tag/content search.
3. Inspect promising results with `pkms get <target> --links`.
4. Use `pkms tags` or `pkms stats --hubs` to find broader entry points.
5. Use `pkms suggest <uuid>` after resolving an exact UUID.
6. Use `pkms mentions <uuid> --incoming` to find notes that name a note
   without linking to it.

### Build Research Context

1. Find candidate notes with `resolve`, `query`, tags, or hubs.
2. Read relevant notes with `get --links`.
3. Use `get <target> --links` to gather neighboring note detail when the output is for an LLM.
4. Summarize findings with citations to note titles/UUIDs when useful.

### Validate and Maintain

1. Run `pkms validate <target>` after editing one note.
2. Run focused checks for the risk being handled, such as `check --id-links`,
   `check --self-links`, or `check --filetags`.
3. Run `pkms check` after a batch of changes.

### Fix Broken ID Links

Do not blindly replace a broken UUID. First inspect the source link text to
understand the intended target.

1. Run `pkms check --id-links`.
2. Open or inspect the source note and find the `[[id:...][description]]`.
3. Resolve the intended target from the description with `resolve`/`query`.
4. Dry-run `pkms fix uuid <broken-full-uuid> <replacement-full-uuid>`.
5. Apply only when the mapping is unambiguous.
6. Verify with `pkms check --self-links --id-links`.

`fix uuid` requires full UUIDs for both arguments.

### Work With Tasks

Use `task list` and `task agenda` for local org task views. The `Id` column is
the bare PKMS task ID accepted by ID-first task commands.

```bash
pkms task list --columns Id,Date,Prio,Note,Heading
pkms task agenda today
pkms task 5 show
pkms task 5 open
pkms task 5 state WAITING
pkms task 5 done --dry-run
```

See the task reference before changing task workflows.

### Create and Link Notes

Use `pkms new` for new note boilerplate and UUID generation. To create a note
with content in one step, pipe the body (without header) via
`pkms new "Title" --create --body -` or pass `--body <path>`. Use full dashed
UUIDs in org links:

```org
[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa][description]]
```

To find link candidates, run `pkms mentions <uuid> --output-format ndjson`
after writing, or `pkms mentions - < draft.org` before the note exists. Do not
resolve candidate terms one by one. Link only records whose sentence justifies
the relation.

For heading anchors, add the heading first, then run:

```bash
pkms new "Existing Note" --create --heading "Heading Title"
```

To split an existing heading subtree into its own note, dry-run first and then
apply:

```bash
pkms extract <heading-full-uuid>
pkms extract <heading-full-uuid> "New Note Title" --apply
```

Validate changed notes afterward.

## Editing Safety

- Inspect user-provided example files before broader analysis.
- Prefer existing notes and aliases over creating approximate links.
- Use inline links only where the surrounding sentence justifies the relation.
- Do not mechanically add backlinks.
- Preserve org heading hierarchy and existing file style.
- After note edits, run `validate` on changed notes and a focused `check`.

## Pipelining Rules

Use NDJSON for command chaining. Producers emit one JSON object per line;
consumers read UUID/path targets from stdin.

Common producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`,
`rag search`, `rag retrieve`.
Common consumers: `get`, `suggest`, `validate`, `task list`.

Use `--from-stdin` when the consumer also has other flags or scope could be
ambiguous.

`orphans` excludes daily notes by default. Use `orphans --with-dailies` only
when explicitly auditing daily notes or debugging orphan-count differences.

For `resolve`, `query`, `orphans`, `suggest`, `rag search`, `rag retrieve`, and
`tags suggest`, narrow discovery with `--include-tags`, `--exclude-tags`,
`--path-prefix`, `--without-dailies`, or `--modified-since` when the requested
scope is known. Included tags all match exactly; any excluded tag rejects a
note. Do not combine `--with-dailies` and `--without-dailies`.
