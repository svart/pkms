# pkms

A CLI tool for navigating, managing, and validating org-roam personal knowledge management systems.

**Design principle: stateless single-run tool.** Each invocation reads the org files from disk, performs
the requested operation, prints output, and exits. No state is persisted between runs — no cache file,
database, daemon, watch mode, or server. This keeps the tool simple, predictable, and easy to debug.
Future development must preserve this stateless model. If state is needed (e.g., for performance),
it must be recomputed from the org files on every run rather than loaded from a persistent cache.

## Install

```bash
cargo install --path .
```

Or run directly:

```bash
cargo run -- <args>
```

## Configuration

`pkms` reads `~/.config/pkms.toml` for persistent settings:

```toml
# Path to the org-roam database root
db_root = "/home/user/Documents/org"

# Directory where new notes are created (relative to db_root or absolute)
new_notes_dir = "roam"

# Glob patterns to skip during file discovery
ignore_patterns = [".attach", "*.bak"]
```

The `--db` flag overrides `db_root` from the config. If neither is provided, the tool errors with instructions.

## Global Flags

| Flag                    | Description                                       |
|-------------------------|---------------------------------------------------|
| `--db PATH`             | Path to org-roam database root (overrides config) |
| `--output-format`       | Output format: `json` or `ndjson`                 |
| `--output-format FMT`   | Output format: `json` or `ndjson`                 |

## Commands

### Fast UUID Resolution

```
pkms resolve --uuid <uuid>              # Search by UUID (substring)
pkms resolve --title <title>            # Search by title or alias (substring)
pkms resolve --tags "tag1,tag2"         # Filter by filetags (substring)
pkms resolve --uuid <uuid> --fields uuid,title,path  # Select output fields
pkms resolve --title <term> --limit 20  # Cap results
```

`resolve`returns UUID, title, path, filetags, and aliases.

### Health & Validation

```
pkms check                              # Full database health scan
pkms check --file-links                 # Check file: links exist on disk
pkms check --attachment-links           # Check attachment: links on disk
pkms validate <target>                  # Validate a specific note
```

`check` scans all notes, validates IDs/titles, detects broken links,
orphans, duplicates, and reports statistics. Returns exit code 1 if
issues are found.

`validate` checks a single note: UUID format, title presence, all
outgoing links (internal + file existence), and lists backlinks.

### Graph Navigation

```
pkms get <target>                          # Retrieve note (content shown by default)
pkms get <target> --links                  # Show note with forward/backward links
pkms get <target> --links --no-content     # Links only, no content
pkms path <from> <to>                      # Shortest path between notes
pkms path <from> <to>                      # Shortest path between notes
```

`get` traverses the link graph up to N hops, showing forward links
and backlinks at each depth. Content is shown by default; use
`--no-content` to suppress it.

`path` finds the shortest connection through the directed graph using
BFS, traversing both outgoing and incoming links.

### Search & Query

```
pkms query "search terms"                 # Fuzzy search titles + content
pkms query "search terms" --tag book      # Filter by filetag
pkms query "search terms" --limit 5       # Limit results
```

`query` searches note titles, aliases, filetags, refs, and content.
Results are scored and sorted by relevance.

### Statistics & Introspection

```
pkms stats                     # Comprehensive database statistics
pkms stats --days 30           # Include recently modified notes
pkms stats --hubs              # Show most-connected notes
pkms stats --hubs 20           # Show top 20 hubs
pkms stats --tags              # List all filetags with counts
pkms orphans                   # List notes with no links
pkms broken                    # List all dangling/broken links
```

`stats` shows total notes, links breakdown, orphans, broken links,
and disk size. Pass `--hubs` or `--tags` for additional detail.

### Fix Issues

```
pkms fix <broken-uuid> <replacement>           # Dry-run (shows what would change)
pkms fix <broken-uuid> <replacement> --apply   # Actually apply replacements
pkms suggest <uuid>                            # Find related notes (takes UUID only)
pkms suggest <uuid> --limit 5                  # Limit suggestions
```

`fix` replaces all occurrences of a broken UUID across the database with
a resolved UUID. Without `--apply` it runs as a dry-run, showing which
files would be modified and how many replacements would be made.

`suggest` takes a note UUID, loads the full graph, and scores every other
note against the target using multi-factor scoring (shared tags, backlinks,
content keyword overlap, directory proximity, title keyword overlap).
Outputs the top N most relevant notes with per-factor breakdowns.

### AI Integration

```
pkms context <target> --depth 2                     # Build AI context window
pkms context <target> --depth 1 --max-tokens 2000   # With token budget
```

`context` produces a formatted text with the note's full content and
linked neighbors at each depth, suitable for LLM consumption.
`--max-tokens` truncates output to fit within the token budget.

### Note Creation

```
pkms new "My Note"                         # Dry-run (just shows filename/UUID)
pkms new "My Note" --create                # Write boilerplate file
pkms new "My Note" --create --tags "tag1,tag2"  # With filetags
pkms new "My Note" --create --aliases "Alias1,Alias2"  # With aliases
```

`new` generates a UUID v4 and a timestamped filename
(`YYYYMMDDHHMMSS-slug.org`) in the configured `new_notes_dir`.
Without `--create`, it only prints the generated values (dry-run).

### Configuration

```
pkms info                          # Show resolved configuration
pkms init-config                   # Generate default config file
pkms init-config --db ~/Documents/org  # With db_root pre-filled
```

### All Commands

| Command       | Description                                      |
|---------------|--------------------------------------------------|
| `check`       | Full database health scan                        |
| `validate`    | Validate a specific note                         |
| `stats`       | Comprehensive database statistics (+ --hubs, --tags) |
| `orphans`     | List orphan notes (no links)                     |
| `broken`      | List broken/dangling links                       |
| `resolve`     | Fast UUID/title resolution (header-only scan)    |
| `fix`         | Replace broken UUIDs across all files            |
| `suggest`     | Find related notes by multi-factor scoring       |
| `context`     | Build AI context window                          |
| `get`         | Retrieve note with neighbors                     |
| `path`        | Shortest path between two notes                  |
| `query`       | Fuzzy search titles and content                  |
| `new`         | Generate filename/UUID for a new note            |
| `info`        | Show current configuration                       |
| `init-config` | Generate default config file                     |

## JSON Output

Every command supports `--output-format json` (or `ndjson`) for structured,
machine-parseable output. This is designed for AI agent consumption:

```bash
pkms --db ~/Documents/org --output-format json check
pkms --db ~/Documents/org --output-format json query "rust"
pkms --db ~/Documents/org --output-format json stats
pkms --db ~/Documents/org --output-format json context "note title" --depth 2
```

JSON Schema files for every command's JSON output are in
[`schemas/`](./schemas/) — one schema per command (`check.json`,
`stats.json`, `resolve.json`, `query.json`, `suggest.json`,
`context.json`). These define the exact structure and types for
reliable programmatic consumption.

## Database Format

The tool expects an [org-roam](https://www.orgroam.com/) directory with:

- Notes named `YYYYMMDDHHMMSS-slug.org` with UUID v4 `:ID:` properties
- Titles via `#+title:` keyword (case-insensitive)
- File-level tags via `#+filetags: :tag1:tag2:`
- Internal links: `[[id:<uuid>][description]]`
- File/URL/attachment links also recognized
- Subdirectories: `roam/`, `roam/common/`, `roam/personal/`, `roam/biblio/`

## Exit Codes

| Code | Meaning                   |
|------|---------------------------|
| 0    | Success / healthy         |
| 1    | Issues found / error      |

## Development

```bash
cargo test                    # Run unit + integration tests
cargo run -- --db <path> <command>  # Test against real database
```

See [TODO.md](./TODO.md) for the implementation roadmap.
