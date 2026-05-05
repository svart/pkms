# pkms Pipelining — Chaining Commands via Unix Pipes

## When to Use

Pipelining lets you chain multiple `pkms` commands together, flowing a set of notes
from one command to the next. This unlocks research workflows that would otherwise
require manual UUID copying.

### Producers → Consumers

| Role | Commands |
|------|----------|
| **Producer** (emits `uuid` per line) | `resolve --output-format ndjson`, `query --output-format ndjson`, `orphans --output-format ndjson`, `stats --hubs --output-format ndjson`, `suggest --output-format ndjson` |
| **Consumer** (reads `uuid` from stdin) | `get`, `suggest`, `validate`, `context` |

A consumer auto-detects a pipe when no target argument is given and stdin is not a TTY.
Use `--from-stdin` to explicitly read from stdin even when other flags are present.

## Full Pipeline Catalog

### Search → Deep Dive (core research workflow)

```bash
pkms query "distributed systems" --output-format ndjson | pkms get --links
```

Finds all notes matching "distributed systems", then for each shows full content
and immediate neighbors. Great for exploring a topic area.

### Tag Browse → Examine Subarea

```bash
pkms resolve --tags "ai" --output-format ndjson | pkms get --links
```

Lists all notes tagged `ai`, then retrieves each with its connections.

### Hubs → Graph Structure

```bash
pkms stats --hubs --output-format ndjson | pkms get --links --no-content
```

Shows the most-connected notes and their immediate graph without content.

### Search → Suggest → Explore (triple pipeline)

```bash
pkms query "concurrency" --output-format ndjson |
  pkms suggest --output-format ndjson |
  pkms get --links
```

Search for "concurrency", get suggestions for every result, then fetch each
suggestion with its neighbors. This is the most powerful research chain.

### Tag Group → Cross-Pollinate Suggestions

```bash
pkms resolve --tags "ml,rust" --output-format ndjson | pkms suggest
```

Takes all notes tagged `ml` or `rust`, then suggests connections for each.
Useful for discovering cross-topic links.

### Orphans → Inspect for Linking

```bash
pkms orphans --output-format ndjson | pkms get --links --no-content
```

Lists every orphan and shows what they'd connect to. Use to decide which orphans
to link into the graph.

### Batch Validate Search Results

```bash
pkms query "foo" --output-format ndjson | pkms validate
```

Runs validation on every note matching "foo" — confirms UUIDs exist, titles
are present, links resolve.

### Resolve → Suggest (cross-pollinate search hits)

```bash
pkms resolve --title "machine learning" --output-format ndjson | pkms suggest --limit 3
```

Finds notes with "machine learning" in the title, then suggests 3 related notes for each.

### Suggest → Get → No Content (link structure only)

```bash
pkms suggest "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa" --limit 5 --output-format ndjson |
  pkms get --no-content
```

Takes 5 suggestions for a given note and shows only their connection graph.

### Stats Hubs → Suggest (discover connections for hubs)

```bash
pkms stats --hubs 5 --output-format ndjson | pkms suggest --limit 3
```

Takes the top 5 hubs and suggests 3 connections for each.

### JSON Output for External Tools

All commands support `--output-format json` for a single structured response.
Pipelining uses `ndjson` (one object per line) so consumers can process
line-by-line.

```bash
pkms query "typescript" --output-format ndjson | pkms validate --output-format json
```

## Implementation Details

- **Auto-detect**: When a consumer command's target is omitted and stdin is a pipe,
  it automatically reads NDJSON from stdin. No `--from-stdin` flag needed.
- **Explicit flag**: Use `--from-stdin` when you also need to specify other flags
  (like `--depth` for `context`, or `--limit` for `suggest`).
- **NDJSON per result**: Consumers output one JSON object per input UUID per line
  when `--output-format ndjson` is set.
- **Full graph reload**: Each command in a pipe loads the full graph independently.
  This matches pkms's stateless architecture. Filesystem caching keeps it fast.

## See Also

- [`get` reference](get.md) — neighbor retrieval in pipe mode
- [`suggest` reference](suggest.md) — suggestion scoring in pipe mode
- [`validate` reference](validate.md) — note validation in pipe mode
- [`context` reference](context.md) — context building in pipe mode
- [`resolve` reference](resolve.md) — fast UUID/title/tag lookup
- [`query` reference](query.md) — fuzzy content search
- [`orphans` reference](orphans.md) — orphan listing
