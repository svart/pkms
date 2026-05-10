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

Build with embedding-based semantic search support (requires the `embed` feature):

```bash
cargo install --path . --features embed
```
```

Or run directly:

```bash
cargo run -- <args>
```

### Install with Nix

Install from a local checkout:

```bash
# Default (no embedding)
nix profile install .#default

# With embedding-based semantic search
nix profile install .#embed
```

Both commands make `pkms` available on your PATH globally.

Install directly from GitHub (without cloning):

```bash
nix profile install github:svart/pkms
nix profile install github:svart/pkms#embed
```

Run or build without installing:

```bash
nix run . -- <args>            # Run default build
nix run .#embed -- <args>      # Run with embedding
nix build                      # Build to ./result
nix build .#embed              # Build with embedding
```

Enter the development shell:

```bash
nix develop
```

Uninstall:

```bash
nix profile list | grep pkms   # find the index
nix profile remove <index>
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

# Agenda configuration: define recognized TODO state keywords
# Open states represent in-progress items, closed states represent completed items.
[agenda]
open_todo_states = ["TODO", "WAITING", "IN-PROGRESS"]
closed_todo_states = ["DONE"]
```

The `--db` flag overrides `db_root` from the config. If neither is provided, the tool errors with instructions.

## Global Flags

| Flag                    | Description                                       |
|-------------------------|---------------------------------------------------|
| `--db PATH`             | Path to org-roam database root (overrides config) |
| `--output-format FMT`   | Output format: `text`, `json`, or `ndjson`        |

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
pkms check                              # Full database health scan (all sections)
pkms check --stats                      # Show database statistics only
pkms check --file-links                 # Check file: links exist on disk
pkms check --attachment-links           # Check attachment: links on disk
pkms check --id-links                   # Check id: links resolve
pkms check --filetags                   # Check filetags format

pkms validate <target>                  # Validate a specific note
```

`check` scans all notes, validates IDs/titles, detects broken links,
orphans, duplicates, and reports statistics. Returns exit code 1 if
issues are found.

Each section (`stats`, `id-links`, `file-links`, `attachment-links`,
`filetags`) has its own flag. When no flags are given,
all sections are shown. When specific flags are given, only those sections
are shown — useful for focused scans or automation.

`validate` checks a single note: UUID format, title presence, all
outgoing links (internal + file existence), and lists backlinks.

### Graph Navigation

```
pkms get <target>                          # Retrieve note (content shown by default)
pkms get <target> --links                  # Show note with forward/backward links
pkms get <target> --headings               # Show heading structure only
pkms get <target> --links --no-content     # Links only, no content
pkms path <from> <to>                      # Shortest path between notes
```

`get` retrieves a note's full content. Use `--links` to show forward
and backward links at depth 1. Use `--headings` to show the heading
structure (level, title, todo state, tags). Content is shown by default;
use `--no-content` to suppress it.

`path` finds the shortest connection through the directed graph using
BFS, traversing both outgoing and incoming links.

### Search & Query

```
pkms query "search terms"                 # Fuzzy search titles + content
pkms query "search terms" --tags          # Search only in filetags
pkms query "search terms" --limit 5       # Limit results
pkms query "search terms" --embed         # Embedding-based semantic search
```

`query` searches note titles, aliases, filetags, refs, and content.
Results are scored and sorted by relevance. Use `--embed` for semantic
similarity search via a local embedding model (`--embed` requires building
with the `embed` feature).

### Statistics & Introspection

```
pkms stats                     # Comprehensive database statistics
pkms stats --days 30           # Include recently modified notes
pkms stats --hubs              # Show most-connected notes
pkms stats --hubs 20           # Show top 20 hubs
pkms stats --tags              # List all filetags with counts
pkms orphans                   # List notes with no links (excludes daily notes)
pkms orphans --with-dailies    # Include daily notes (filename matching YYYY-MM-DD.org)
```

`stats` shows total notes, links breakdown, orphans, broken links,
and disk size. Pass `--hubs` or `--tags` for additional detail.

### Fix Issues

```
pkms fix <broken-uuid> <replacement-uuid>           # Dry-run (shows what would change)
pkms fix <broken-uuid> <replacement-uuid> --apply   # Actually apply replacements
pkms suggest <uuid>                            # Find related notes (takes UUID only)
pkms suggest <uuid> --limit 5                  # Limit suggestions
pkms suggest <uuid> --embed                    # Embedding-based semantic suggestions
```

`fix` replaces all occurrences of a broken UUID across the database with
a replacement UUID. Both arguments must be full UUIDs (with dashes).
Without `--apply` it runs as a dry-run, showing which
files would be modified and how many replacements would be made.

`suggest` takes a note UUID, loads the full graph, and scores every other
note against the target using multi-factor scoring (shared tags, backlinks,
content keyword overlap, directory proximity, title keyword overlap,
neighborhood relevance). Use `--embed` for embedding-based semantic similarity
via a local BGE-small-en-v1.5 model (`--embed` requires building with the
`embed` feature; the model is downloaded on first use to ~/.cache/pkms/).

### AI Integration

```
pkms context <target> --depth 2                          # Build AI context window
pkms context <target> --depth 1 --max-tokens 2000        # With token budget
pkms context <target> --encoding o200k_base              # Use GPT-4o tokenizer
```

`context` produces a formatted text with the note's full content and
linked neighbors at each depth, suitable for LLM consumption.
`--max-tokens` truncates output to fit within the token budget.
`--encoding` selects the tokenizer (cl100k_base for GPT-4, o200k_base for GPT-4o).
Token counts use accurate BPE encoding via tiktoken-rs.

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

| Command       | Description                                               |
|---------------|-----------------------------------------------------------|
| `check`       | Full database health scan (+ --stats, --file-links, etc.)  |
| `validate`    | Validate a specific note                                  |
| `stats`       | Comprehensive database statistics (+ --hubs, --tags)      |
| `orphans`     | List orphan notes (no links, + --with-dailies)            |
| `resolve`     | Fast UUID/title resolution (header-only scan)             |
| `fix`         | Replace broken UUIDs across all files                     |
| `suggest`     | Find related notes (+ --embed for semantic similarity)    |
| `context`     | Build AI context window (+ --encoding for tokenizer)      |
| `get`         | Retrieve note with neighbors (shows content tokens)       |
| `path`        | Shortest path between two notes                           |
| `query`       | Fuzzy search titles and content (+ --embed for semantic)  |
| `new`         | Generate filename/UUID for a new note                     |
| `info`        | Show current configuration                                |
| `init-config` | Generate default config file                              |

## JSON Output

Every command supports `--output-format json` (or `ndjson`) for structured,
machine-parseable output. This is designed for AI agent consumption:

```bash
pkms --db ~/Documents/org --output-format json check
pkms --db ~/Documents/org --output-format json query "rust"
pkms --db ~/Documents/org --output-format json stats
pkms --db ~/Documents/org --output-format json context "note title" --depth 2
```

JSON or NDJSON output is designed for reliable programmatic consumption
by AI agents and scripting pipelines.

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

See [TODO.md](./TODO.md) for the development roadmap.
