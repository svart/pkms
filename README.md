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
| `--json`                | Structured JSON output for AI/script consumption  |
| `--output-format FMT`   | Output format: `json` or `ndjson`                 |
| `-v`                    | Verbose output during processing                  |
| `-q` / `--quiet`        | Suppress non-essential stderr output              |
| `--no-header`           | Suppress column headers in human output           |
| `--count`               | Show only the count of results                    |

## Commands

### Fast UUID Resolution

```
pkms resolve <search-term>              # Quick header-only scan
pkms resolve --search <substring>       # Search titles/aliases
pkms resolve --tags "tag1,tag2"         # Filter by filetags
pkms resolve --limit 20                 # Cap results
pkms resolve "uuid" --fields uuid,title,path  # Select output fields
```

`resolve` scans only file headers (~0.5s for 760 files). Returns UUID,
title, path, filetags, and aliases. Use for quick lookups before heavy
operations.

### Health & Validation

```
pkms check                              # Full database health scan
pkms check --file-links                 # Check file: links exist on disk
pkms check --attachment-links           # Check attachment: links on disk
pkms validate <target>                  # Validate a specific note
pkms validate --input-json params.json  # Load target from JSON file
```

`check` scans all notes, validates IDs/titles, detects broken links,
orphans, duplicates, and reports statistics. Returns exit code 1 if
issues are found.

`validate` checks a single note: UUID format, title presence, all
outgoing links (internal + file existence), and lists backlinks.

### Graph Navigation

```
pkms get <target> --depth 2                # Retrieve note with neighbors
pkms get <target> --depth 1 --graph        # ASCII art visualization
pkms get <target> --out                    # Show full note content
pkms get --from-stdin                      # Read targets from stdin
pkms get --from-file targets.txt           # Read targets from file
pkms get --input-json params.json          # Load params from JSON
pkms path <from> <to>                      # Shortest path between notes
pkms path "A" "B" --max-depth 10           # Limit traversal depth
pkms path --input-json params.json         # Load params from JSON
pkms subgraph <target> --depth 2           # Export subgraph with stats
pkms subgraph --input-json params.json     # Load params from JSON
```

`get` traverses the link graph up to N hops, showing forward links
and backlinks at each depth. `--graph` renders a tree visualization.
`--out` shows the full org-mode content of each note.

`path` finds the shortest connection through the directed graph using
BFS, traversing both outgoing and incoming links.

`subgraph` exports all nodes and edges within depth with graph
density statistics (vertex count, edge count, avg order).

### Search & Query

```
pkms query "search terms"                 # Fuzzy search titles + content
pkms query "search terms" --tag book      # Filter by filetag
pkms query "search terms" --limit 5       # Limit results
pkms query --input-json params.json       # Load params from JSON
pkms tags                                 # List all filetags with counts
pkms tags --tag book                      # List notes with a specific tag
```

`query` searches note titles, aliases, filetags, refs, and content.
Results are scored and sorted by relevance.

### Statistics & Introspection

```
pkms stats                     # Comprehensive database statistics
pkms stats --days 30           # Include recently modified notes
pkms orphans                   # List notes with no links
pkms broken                    # List all dangling/broken links
pkms hubs                      # List most-connected notes
pkms hubs --limit 20           # Show top 20 hubs
```

`stats` shows total notes, links breakdown, orphans, broken links,
disk size, directory breakdown, and top hub nodes.

### Fix Issues

```
pkms fix <broken-uuid> <replacement>           # Dry-run (shows what would change)
pkms fix <broken-uuid> <replacement> --apply   # Actually apply replacements
pkms suggest <target>                          # Find related notes
pkms suggest <target> --limit 5                # Limit suggestions
pkms suggest --input-json params.json          # Load params from JSON
```

`fix` replaces all occurrences of a broken UUID across the database with
a resolved UUID. Without `--apply` it runs as a dry-run, showing which
files would be modified and how many replacements would be made.

`suggest` scores and ranks notes by relevance to the target using
multi-factor scoring (shared tags, backlinks, content keyword overlap,
path similarity, alias matching).

### AI Integration

```
pkms context <target> --depth 2                     # Build AI context window
pkms context <target> --depth 1 --max-tokens 2000   # With token budget
pkms context <target> --include-outgoing false       # Skip forward links
pkms context <target> --include-incoming false       # Skip backlinks
pkms context <target> --template "{{title}}: {{content}}"  # Custom template
pkms context --input-json params.json               # Load params from JSON
```

`context` produces a formatted text with the note's full content and
linked neighbors at each depth, suitable for LLM consumption.
`--max-tokens` truncates output to fit within the token budget.
`--template` allows custom formatting with `{{title}}`, `{{content}}`,
`{{neighbors}}`, and `{{backlinks}}` placeholders.

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
| `stats`       | Comprehensive database statistics                |
| `orphans`     | List orphan notes (no links)                     |
| `broken`      | List broken/dangling links                       |
| `hubs`        | List most-connected notes                        |
| `resolve`     | Fast UUID/title resolution (header-only scan)    |
| `fix`         | Replace broken UUIDs across all files            |
| `suggest`     | Find related notes by multi-factor scoring       |
| `context`     | Build AI context window                          |
| `get`         | Retrieve note with neighbors                     |
| `path`        | Shortest path between two notes                  |
| `subgraph`    | Export subgraph with stats                       |
| `query`       | Fuzzy search titles and content                  |
| `tags`        | List filetags with counts                        |
| `new`         | Generate filename/UUID for a new note            |
| `info`        | Show current configuration                       |
| `init-config` | Generate default config file                     |

## JSON Output

Every command supports `--json` for structured, machine-parseable
output. This is designed for AI agent consumption:

```bash
pkms --db ~/Documents/org --json check
pkms --db ~/Documents/org --json query "rust"
pkms --db ~/Documents/org --json stats
pkms --db ~/Documents/org --json context "note title" --depth 2
```

JSON Schema files for every command's `--json` output are in
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
