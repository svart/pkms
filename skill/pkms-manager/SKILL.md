---
name: pkms-manager
description: Manage and navigate an org-roam PKMS (Personal Knowledge Management System) database of interconnected notes. Use this skill whenever the user wants to search their notes, check database health, fix broken links, retrieve note context for research, create new notes, analyze connections between notes, or perform any operation on their org-roam knowledge base. Trigger when the user mentions PKMS, org-roam, their note database, knowledge base, or references to ~/Documents/org. This skill knows the exact CLI interface including all subcommands, JSON output parsing, and common workflows.
---

# pkms-manager

This skill helps you work with the `pkms` CLI tool to manage an org-roam database.

## Tool location

The `pkms` binary should be available. The org-roam database is at `~/Documents/org`.

## Database

The org-roam database org-mode notes organized as:
- `roam/common/` — technical reference notes (681 files, heavily interlinked)
- `roam/personal/` — personal/life notes (73 files)
- `roam/biblio/` — book annotations
- `roam/` — top-level roam notes
- Root `.org` files — standalone notes, todo, inbox, etc.

Each note has UUID v4 `:ID:` in a property drawer, `#+title:`, and internal links via `[[id:<uuid>][description]]`.

## Global flags

These flags work with every command:

| Flag | Description |
|------|-------------|
| `--db PATH` | Path to org-roam database root (overrides config) |
| `--output-format FMT` | Output format: `text`, `json` or `ndjson` |

## Commands reference

Each command has a detailed reference file in [`references/`](references/).

### `check` — [Full reference](references/check.md)
Full database health scan. Validates all notes, detects broken internal/file/attachment links, duplicate UUIDs/titles, missing titles, and parse errors. Returns exit code 0 if healthy, 1 if issues found. Run after any edits to verify database integrity. Flags: `--file-links`, `--attachment-links`, `--id-links`.

### `validate` — [Full reference](references/validate.md)
Health check for a single note. Verifies UUID format, title presence, outgoing links (internal + file), and lists backlinks. Use after creating or editing a specific note before running a full `check`.

### `stats` — [Full reference](references/stats.md)
Comprehensive database statistics: total notes, link breakdown, orphans, broken links, disk size, and directory distribution. Use `--hubs` for most-connected notes (exploration starting points), `--tags` for filetag browsing, `--days N` for recent activity.

### `orphans` — [Full reference](references/orphans.md)
List notes with no connections — no outgoing internal links and no backlinks. Use to find notes that need linking into the graph.

### `context` — [Full reference](references/context.md)
Build an AI-friendly context window for a note. Includes the note's full content plus linked neighbors at each depth level (configurable via `--depth`). Supports token budget via `--max-tokens` for LLM consumption.

### `resolve` — [Full reference](references/resolve.md)
Fast UUID/title/tag lookup by reading only file headers (first 100 lines). Does NOT load the full graph, making it the fastest search command. Supports substring matching on UUID, title/alias, and filetags with `--fields` and `--limit`.

### `fix` — [Full reference](references/fix.md)
Replace all occurrences of a broken UUID across the entire database. Dry-run by default; use `--apply` to write changes. The broken UUID and replacement target can be full UUIDs, 8-char prefixes, or note titles.

### `suggest` — [Full reference](references/suggest.md)
Find thematically related notes by multi-factor scoring (title overlap, shared tags, shared backlinks/outgoing, content keywords, directory proximity, neighborhood relevance). Accepts exact UUID only — use `resolve` first to find it.

### `new` — [Full reference](references/new.md)
Generate a UUID v4 and timestamped filename (`YYYYMMDDHHMMSS-slug.org`) in the configured new notes directory. Dry-run by default; use `--create` to write the boilerplate file with optional `--tags` and `--aliases`.

### `get` — [Full reference](references/get.md)
Retrieve a note's full content with optional neighbor display (`--links`). Use `--no-content` to show links only. Good for deep exploration of a topic's graph neighborhood.

### `query` — [Full reference](references/query.md)
Fuzzy search across all note content (titles, aliases, refs, tags, file content). Supports scope flags (`--tags`, `--title`, `--content`) and `--limit`. Results are scored and sorted by relevance.

### `path` — [Full reference](references/path.md)
Find the shortest path between two notes through the directed graph using bidirectional BFS. Accepts titles, UUIDs, or file paths. Reports the hop count and ordered path.

### `info` — [Full reference](references/info.md)
Show the resolved configuration: loaded config file, effective db_root, new notes directory, and whether `--db` overrides are active.

## Common workflows

### Note Discovery
1. `resolve --title <term>` for fast targeted lookup.
2. `query "terms"` for broad search.
3. Analyze content of 3-5 relevant notes and their neighbors if necessary (use `get` command with `--links` flag).
4. `stats --tags` to browse by filetag
5. `stats --hubs` to find hub notes
6. `suggest` command for related notes suggestions if query did not bring.

### Research / Context Building
1. Find hubs: `pkms stats --hubs`. Identify related hubs for the research topic.
2. Explore from hub(s) via `get` with `--links`.
3. Use `get` command to analyze the content of the note.
4. Discover connections: `suggest <uuid>`
5. Search: `query` and `resolve`
6. Build context: `context <uuid>` with necessary depth (depends on amount of links for note).

### Database Health Maintenance
1. `check` to see health status and broken links.
2. Apply procedure from "Note Discovery" to find relevant notes for broken links.
3. `fix <broken-uuid> <replacement> --apply` for each broken UUID.
4. Apply procedure from "Linking Orphans to the Graph" for finding and fixing orphans.
5. `check` to verify final state

### Linking Orphans to the Graph
1. `orphans` to list all orphans. Pick ones with clear thematic connections.
2. Apply procedure from "Note Discovery" to find relevant note.
3. **Analyze suggestions** — read the orphan's content and at least 5-8 top suggestions to confirm connections are real.
4. If suggestions still not so relevant you may create "adoption" note to smoothly connect current orphan to the graph. Fill new note with short portion of relevant information.
4. Always **inline links** when content exists: embed `[[id:<full-uuid>][description]]` into existing sentences. E.g. "A systematic framework for technical [[id:63649b3f-5168-4fdc-96ec-1911a91b54a5][documentation]] authoring." If content does not exist, create highly relevant content for the note.
5. **Use full UUIDs** (dashed format), not short 8-char UUIDs. Every link must match the `:ID:` property exactly.
6. **Backlinks sparingly** — only add a link *from* an existing note *to* the orphan when there is genuine contextual reason (shared topic, direct dependency, natural cross-reference). Do not mechanically pair every forward link with a backlink.
7. If mentioning orphan in already existing note is natural just create this link without adding direct forward link from orphan.
8. `validate <uuid>` each changed note to confirm no broken links.
9. `check` to verify overall database health.. `check` to verify overall health.

### Performance Notes

All subcommands are very fast even on large databases.
