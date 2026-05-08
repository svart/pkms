---
name: pkms-manager
description: Manage and navigate an org-roam PKMS (Personal Knowledge Management System) database of interconnected notes. Use this skill whenever the user wants to search their notes, check database health, fix broken links, retrieve note context for research, create new notes, analyze connections between notes, or perform any operation on their org-roam knowledge base. Trigger when the user mentions PKMS, org-roam, their note database, knowledge base, or references to ~/Documents/org. This skill knows the exact CLI interface including all subcommands, JSON output parsing, and common workflows.
---

# pkms-manager

This skill helps you work with the `pkms` CLI tool to manage an org-roam database.

## Tool location

The `pkms` binary should be available. The database location is configured via `~/.config/pkms.toml` or the `--db` flag.

## Database

The org-roam database contains org-mode notes organized in subdirectories (varies by database — run `pkms info` to see configuration). Each note has UUID v4 `:ID:` in a property drawer, `#+title:`, and internal links via `[[id:<uuid>][description]]`. Headings within notes may also carry `:ID:` properties for direct section-level linking. Use `pkms get <target> --headings` to see heading UUIDs, and `pkms new --heading` to create them. Run `pkms info` as your first command to understand the active configuration.

### Before any workflow

Always start by verifying the configuration:
```bash
pkms info
```
This shows the active db_root, new_notes_dir, and whether a config file is loaded. If the database root is wrong, pass `--db /actual/path` to any command.

## Global flags

These flags work with every command:

| Flag | Description |
|------|-------------|
| `--db PATH` | Path to org-roam database root (overrides config) |
| `--output-format FMT` | Output format: `text`, `json` or `ndjson` |

## Commands reference

Each command has a detailed reference file in [`references/`](references/).

### `check` — [Full reference](references/check.md)
Full database health scan.
Validates all notes, detects broken internal/file/attachment links, duplicate UUIDs/titles, missing titles, and parse errors.
Returns exit code 0 if healthy, 1 if issues found.
Run after edits to verify database integrity.

Each section (stats, id-links, file-links, attachment-links, filetags,
heading-backlinks) has its own flag. When no flags are given, all sections
are shown. When specific flags are given, only those sections are shown.

| Command | Usecase |
|---------|---------|
| `pkms check` | Full health scan: all sections shown. |
| `pkms check --stats` | Show database statistics (notes, links, orphans, broken counts). |
| `pkms check --id-links` | Verify only that links resolve to valid UUIDs in the database. |
| `pkms check --file-links` | Check only that `file:` link targets exist on disk. |
| `pkms check --attachment-links` | Check only that `attachment:` link targets exist on disk. |
| `pkms check --filetags` | Verify that all `#+filetags:` lines use the canonical `:tag1:tag2:tag3:` format. |
| `pkms check --heading-backlinks` | Check only for heading-level backlinks. |

### `validate` — [Full reference](references/validate.md)
Health check for a single note.
Verifies UUID format, title presence, outgoing links.
Use after creating or editing a specific note.

| Command | Usecase |
|---------|---------|
| `pkms validate <UUID>` | Check a single note UUID |

### `stats` — [Full reference](references/stats.md)
Comprehensive database statistics: total notes, link breakdown, orphans, broken links, disk size.

| Command | Usecase |
|---------|---------|
| `pkms stats` | Show full statistics: notes, links, orphans, broken links, disk size. |
| `pkms stats --hubs` | List top 10 most-connected hub notes (exploration starting points). |
| `pkms stats --hubs 20` | List top 20 most-connected hub notes. |
| `pkms stats --tags` | Browse all filetags with note counts. |

### `orphans` — [Full reference](references/orphans.md)
List notes with no connections — no outgoing internal links and no backlinks.

| Command | Usecase |
|---------|---------|
| `pkms orphans` | List notes with no incoming or outgoing internal links. |

### `resolve` — [Full reference](references/resolve.md)
Fast notes lookup by metadata.

| Command | Usecase |
|---------|---------|
| `pkms resolve --uuid <UUID>` | Find a note by any :ID: (note-level or heading-level, substring match) |
| `pkms resolve --title <title>` | Look up notes by title or alias substring |
| `pkms resolve --title "graph"` | Look up notes with "graph" substring in title (e.g "subgraph", "graphs", etc.) |
| `pkms resolve --title "gr alg"` | Look up notes with "gr" and "alg" substrings in title (e.g "graph algorithms", "algorithms on graphs", etc.) |
| `pkms resolve --tags "tagname"` | Filter notes by tag. Substring matching. |
| `pkms resolve --tags "tag1,tag2"` | Filter notes by multiple tags (comma-separated). Substring matching. At least one tag should match to get result. |

### `get` — [Full reference](references/get.md)
Retrieve a note's full content with optional neighbor display.

| Command | Usecase |
|---------|---------|
| `pkms get <target>` | Retrieve note content |
| `pkms get <target> --links` | Show note content with forward and backward neighbors |
| `pkms get <target> --links --no-content` | Show only list of links |
| `pkms get <target> --headings --no-content` | Show only heading structure |
| `pkms get <target> --links --headings --no-content` | Show links and heading structure without content |

`<target>` may be UUID, title or absolute file path.

### `path` — [Full reference](references/path.md)
Find the shortest path between two notes through the notes graph.
Accepts titles, UUIDs, or absolute file paths.
Good for checking if notes are connected via well-defined logic.


| Command | Usecase |
|---------|---------|
| `pkms path <from> <to>` | Find shortest path between two notes via BFS |

### `query` — [Full reference](references/query.md)
Fuzzy search across all note content (titles, aliases, refs, tags, file content).
Results are scored and sorted by relevance.

| Command | Usecase |
|---------|---------|
| `pkms query "term"` | Fuzzy search titles, aliases, refs, tags, and file content |
| `pkms query "term" --tags` | Search only within filetags. |
| `pkms query "term" --limit N` | Show not more than N matching notes. |
| `pkms query "term" --title` | Search only in titles, aliases, and refs. |
| `pkms query "term" --embed` | Semantic search via embeddings. |

If you what to search multiple terms enclose them into quotes and separate by spaces: `pkms query "term1 term2"`

### `fix` — [Full reference](references/fix.md)
Replace all occurrences of a broken UUID across the entire database. Both arguments must be full UUIDs (with dashes). Dry-run by default; use `--apply` to write changes.

| Command | Usecase |
|---------|---------|
| `pkms fix <broken_uuid> <replacement_uuid>` | Dry-run: preview which files and how many replacements would be made |
| `pkms fix <broken_uuid> <replacement_uuid> --apply` | Apply broken UUID replacement in all files |

### `suggest` — [Full reference](references/suggest.md)
Find thematically related notes by multi-factor scoring
- title overlap;
- shared tags;
- shared backlinks/outgoing;
- content keywords;
- directory proximity;
- neighborhood relevance.

| Command | Usecase |
|---------|---------|
| `pkms suggest <uuid>` | Find related notes by multi-factor scoring. |
| `pkms suggest <uuid> --embed` | Semantic suggestions via embeddings. |

### `context` — [Full reference](references/context.md)
Build an AI-friendly context window for a note.
Includes the note's full content plus linked neighbors at each depth level.

| Command | Usecase |
|---------|---------|
| `pkms context <target> --depth N` | Build context window with linked neighbors up to depth N |
| `pkms context <target> --depth N --max-tokens M` | Build context with token budget for LLM consumption |

`<target>` may be UUID, title or absolute file path.

### `new` — [Full reference](references/new.md)
Generate a UUID v4 and timestamped filename (`YYYYMMDDHHMMSS-slug.org`) in the configured new notes directory.

| Command | Usecase |
|---------|---------|
| `pkms new "Title"` | Dry-run: preview generated UUID, filename, and path. |
| `pkms new "Title" --create` | Create boilerplate note file on disk. |
| `pkms new "Title" --create --tags "tag1,tag2"` | Create note with filetags. |
| `pkms new "Title" --create --heading "Heading 1"` | Generate heading-level `:ID:` for an existing heading in the note |

### `info` — [Full reference](references/info.md)
Show the configuration with notes database path.

| Command | Usecase |
|---------|---------|
| `pkms info` | Show config.  |

## Common workflows

### Note Discovery
1. `pkms resolve --title <term>` for fast targeted lookup.
2. `pkms query "term"` for broad search.
3. Analyze content of 3-5 relevant notes and their neighbors if necessary (use `pkms get`).
4. `pkms stats --tags` to browse by filetag.
5. `pkms stats --hubs 20` to find hub notes.
6. `pkms suggest <UUID>` command for related notes suggestions if query did not bring.

### Research / Context Building
1. Find hubs: `pkms stats --hubs 20`. Identify related hubs for the research topic.
2. Explore relevant for the topic hub via `pkms get <UUID>`.
3. Find relevant information in the hub, follow the links using `pkms get <UUID>`.
4. Discover relevan notes using `pkms suggest <UUID>`.
5. Search for specific terms across the database: `pkms query` and `pkms resolve` to find more relevant notes.
6. Collect information about the topic create summary then propose user what to do next in your research by providing 3-4 variants.

### Database Health Maintenance
1. `check` to see health status and broken links.
2. Apply procedure from "Note Discovery" to find relevant notes for broken links.
3. `fix <broken-uuid> <replacement> --apply` for each broken UUID.
4. Apply procedure from "Linking Orphans to the Graph" for finding and fixing orphans.
5. `check` to verify final state

### Linking Orphans to the Graph
1. `orphans` to list all orphans. Pick ones with clear thematic connections.
2. If orphaned note fills empty. Fill it with minimal necessary information.
3. Apply procedure from "Note Discovery" to find relevant note.
4. **Analyze suggestions** — read the orphan's content and at least 5-8 top suggestions to confirm connections are real.
5. If suggestions still not so relevant you may create "adoption" note to smoothly connect current orphan to the graph. Fill new note with short portion of relevant information.
6. Always **inline links** when content exists: embed `[[id:<full-uuid>][description]]` into existing sentences. E.g. "A systematic framework for technical [[id:63649b3f-5168-4fdc-96ec-1911a91b54a5][documentation]] authoring." If content does not exist, create highly relevant content for the note.
7. **Use full UUIDs** (dashed format), not short 8-char UUIDs. Every link must match the `:ID:` property exactly.
8. **Backlinks sparingly** — only add a link *from* an existing note *to* the orphan when there is genuine contextual reason (shared topic, direct dependency, natural cross-reference). Do not mechanically pair every forward link with a backlink.
9. If mentioning orphan in already existing note is natural just create this link without adding direct forward link from orphan.
10. `validate <uuid>` each changed note to confirm no broken links.
11. `check` to verify overall database health.. `check` to verify overall health.

### Creating and Linking Heading-Level IDs

When a topic within a note deserves its own anchor point for cross-linking:

1. Create the note with heading (if note is not available):
   `pkms new "Note name" --create`
2. Add the heading manually to the note file (e.g. `* Sub Topic`).
3. Generate heading-level `:ID:`:
   `pkms new "Note name" --create --heading "Sub Topic"`
4. Find heading UUIDs in an existing note or get heading UUID from previous `new` command output:
   `pkms get <target> --headings --no-content --output-format json`
5. Link to a specific heading from another note:
   `[[id:<heading-uuid>][contextual text here]]`
6. Verify both notes with `pkms validate <uuid>`.

## Command Pipelining

Commands can be chained via Unix pipes using NDJSON.
Producers emit per-item lines with a field; consumers read them from stdin via automatic pipe detection or `--from-stdin`.

**Producers** (emit with `--output-format ndjson`): `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`
**Consumers** (read via pipe or `--from-stdin`): `get`, `suggest`, `validate`, `context`

Always use `--output-format ndjson` for commands that pass data to other commands in pipeline.

### Key pipelines

```bash
# Search → deep dive (core research)
pkms query "distributed systems" --output-format ndjson | pkms get --links

# Tag browse → examine subarea
pkms resolve --tags "ai" --output-format ndjson | pkms get --links

# Orphans → inspect for linking
pkms orphans --output-format ndjson | pkms get --links --no-content

# Search → suggest → explore (triple pipeline)
pkms query "concurrency" --output-format ndjson |
  pkms suggest --output-format ndjson |
  pkms get --links

# Tag group → cross-pollinate suggestions
pkms resolve --tags "ml,rust" --output-format ndjson | pkms suggest

# Batch validate search results
pkms query "foo" --output-format ndjson | pkms validate
```

See [Pipelining reference](references/pipelining.md) for the full catalog of examples.

## Performance Notes

All subcommands are very fast even on large databases.
