---
name: pkms-manager
description: Manage and navigate an org-roam PKMS (Personal Knowledge Management System) database of interconnected notes. Use this skill whenever the user wants to search their notes, check database health, fix broken links, retrieve note context for research, create new notes, analyze connections between notes, or perform any operation on their org-roam knowledge base. Trigger when the user mentions PKMS, org-roam, their note database, knowledge base, or references to ~/Documents/org. This skill knows the exact CLI interface including all subcommands, JSON output parsing, and common workflows.
---

# pkms-manager

This skill helps you work with the `pkms` CLI tool to manage an org-roam database.

## Tool location

The `pkms` binary is at `/home/svart/work/my-projects/pkms/target/debug/pkms`. The org-roam database is at `~/Documents/org`. You can also use `cargo run -- <args>` from `/home/svart/work/my-projects/pkms/`.

## Database

The org-roam database at `~/Documents/org` contains ~760 org-mode notes organized as:
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
| `--json` | Structured JSON output |
| `--output-format ndjson` | Newline-delimited JSON output |
| `-v` / `--verbose` | Verbose output |
| `-q` / `--quiet` | Suppress non-essential stderr output |
| `--no-header` | Suppress column headers in human output |
| `--count` | Show only the count of results |
| `--example` | Show usage example for the given command and exit |

## Commands reference

### Fast UUID Resolution (no full parse — ~0.5s)
```
pkms --db ~/Documents/org --json resolve <search-term>
pkms --db ~/Documents/org --json resolve --search <substring>
pkms --db ~/Documents/org --json resolve --tags "tag1,tag2"
pkms --db ~/Documents/org --json resolve --limit 20
pkms --db ~/Documents/org --json resolve "uuid" --fields uuid,title,path
```
Use `resolve` for quick lookups (only reads file headers). Returns UUID, title, path, tags, aliases. `--fields` selects which columns to include, `--limit` caps results.

### Search & Query (full parse — ~3s)
```
pkms --db ~/Documents/org --json query "search terms" --limit 10
pkms --db ~/Documents/org --json query "terms" --tag book
pkms --db ~/Documents/org --json query --input-json params.json
```
### Retrieve Notes
```
pkms --db ~/Documents/org --json get <uuid-or-title> --depth 1
pkms --db ~/Documents/org get <uuid-or-title> --depth 1 --graph   # ASCII tree
pkms --db ~/Documents/org get <uuid-or-title> --out                # Full content
pkms --db ~/Documents/org get --from-stdin                          # Read targets from stdin
pkms --db ~/Documents/org get --from-file targets.txt               # Read targets from file
pkms --db ~/Documents/org get --input-json params.json              # JSON params
```
### Graph Navigation
```
pkms --db ~/Documents/org --json path "note A" "note B"               # Shortest path
pkms --db ~/Documents/org --json path "A" "B" --max-depth 10         # Limit traversal depth
pkms --db ~/Documents/org --json path --input-json params.json       # JSON params
pkms --db ~/Documents/org --json subgraph <uuid> --depth 2            # Subgraph export
pkms --db ~/Documents/org --json subgraph --input-json params.json   # JSON params
```
### Health & Validation
```
pkms --db ~/Documents/org check                                 # Full scan, exit code 1 if issues
pkms --db ~/Documents/org check --file-links                     # Also check file: links exist on disk
pkms --db ~/Documents/org check --attachment-links               # Also check attachment: links exist
pkms --db ~/Documents/org --json validate <target>                # Single note health
pkms --db ~/Documents/org --json validate --input-json params.json
pkms --db ~/Documents/org --json orphans                         # Orphan notes
pkms --db ~/Documents/org --json broken                          # Broken links
pkms --db ~/Documents/org --json stats                           # DB statistics
pkms --db ~/Documents/org --json stats --days 30                 # Recent changes
pkms --db ~/Documents/org --json hubs                            # Most-connected notes
pkms --db ~/Documents/org hubs --limit 5                         # Top 5 hubs
pkms --db ~/Documents/org tags                                   # Filetags with counts
pkms --db ~/Documents/org tags --tag <tag>                       # Notes with a tag
```
### Fix Issues
```
pkms --db ~/Documents/org fix <broken-uuid> <replacement>         # Dry-run
pkms --db ~/Documents/org fix <broken-uuid> <replacement> --apply # Apply
pkms --db ~/Documents/org --json suggest <target>                  # Related notes
pkms --db ~/Documents/org suggest --limit 5 --input-json params.json
```
### Create Notes
```
pkms --db ~/Documents/org new "Title"                    # Dry-run (shows UUID)
pkms --db ~/Documents/org new "Title" --create           # Write file
pkms --db ~/Documents/org new "Title" --create --tags "tag1,tag2"
pkms --db ~/Documents/org new "Title" --create --aliases "alt1,alt2"
```
### AI Context
```
pkms --db ~/Documents/org context <target> --depth 1
pkms --db ~/Documents/org context <target> --depth 2 --max-tokens 2000
pkms --db ~/Documents/org context <target> --include-outgoing false
pkms --db ~/Documents/org context <target> --include-incoming false
pkms --db ~/Documents/org context <target> --template "{{title}}: {{content}}"
pkms --db ~/Documents/org context --input-json params.json
```
### Configuration
```
pkms --db ~/Documents/org info
pkms --db ~/Documents/org --json info
pkms init-config                                          # Generate ~/.config/pkms.toml
pkms init-config --db ~/Documents/org                     # With database root preset
```

## JSON Schemas

Every command that supports `--json` has a corresponding JSON Schema in `schemas/<command>.json` (relative to this skill directory). These schemas define the exact output structure and types for reliable programmatic consumption:

| Command     | Schema file              | Top-level keys |
|-------------|-------------------------|----------------|
| `check`     | `schemas/check.json`    | `db_root`, `stats`, `duplicates`, `broken_links`, `broken_file_links`, `broken_attachment_links`, `failed_files`, `healthy` |
| `stats`     | `schemas/stats.json`    | `db_root`, `total_notes`, `total_links`, `hubs`, `directories`, `recent_notes` |
| `resolve`   | `schemas/resolve.json`  | `query`, `total`, `results[]` (uuid, title, path, filetags, aliases) |
| `suggest`   | `schemas/suggest.json`  | `target`, `target_uuid`, `suggestions[]` (uuid, title, score, scores{}, reasons[]) |
| `query`     | `schemas/query.json`    | `query`, `total_results`, `results[]` (uuid, title, score, matches[], content_matches[]) |
| `context`   | `schemas/context.json`  | `target`, `context`, `estimated_tokens`, `depth` |

## Common workflows

### Research / Context Building
When the user asks about a topic, use this sequence:
1. `resolve` to find the note UUID quickly
2. `context <uuid> --depth 2` to build a rich context window with linked neighbors
3. For deeper exploration, `get <uuid> --out` for full content

### Database Health Maintenance
When the user mentions fixing their database:
1. `check` to see the health status and exit code
2. `broken --json` to list all broken links grouped by target UUID
3. `resolve <concept>` to find the correct replacement note UUID
4. `fix <broken-uuid> <replacement> --apply` for each broken UUID batch
5. `orphans` to find notes needing connections
6. `suggest <orphan-title>` to find related notes for linking

### Note Discovery
When the user wants to find something in their notes:
1. `query "terms" --json --limit 20` for broad search
2. `resolve <term>` for fast targeted lookup
3. `tags` to browse by filetag
4. `hubs` to find highly-connected "hub" notes

### JSON Output Shapes

Always use `--json` for AI consumption. Each command has its own output shape — refer to the JSON schemas for precise field definitions. Common patterns:

- **Single-item commands** (`check`, `stats`, `validate`, `get`, `context`, `fix`, `new`, `path`, `subgraph`, `info`): each returns a top-level object with command-specific keys.
- **List commands** use varying key names for their result arrays:
  - `resolve` returns `{"query", "total", "results": [...]}`
  - `query` returns `{"query", "total_results", "results": [...]}`
  - `orphans` returns `{"count", "orphans": [...]}`
  - `broken` returns `{"count", "links": [...]}`
  - `hubs` returns `{"limit", "hubs": [...]}`
  - `tags` returns `{"tags": [...]}`
  - `suggest` returns `{"target", "target_uuid", "suggestions": [...]}`

### Linking Orphans to the Graph
After `suggest` finds related notes, append links manually:
```bash
echo -e "\n[[id:<target-uuid>][link text]]" >> ~/Documents/org/<path-to-orphan>
```

**Note**: `suggest` works best for notes with descriptive titles and rich
content. For short-content notes (e.g., "CQI", "MCS"), the content keyword
matching may produce noisy results. In those cases, use `resolve <topic>`
to find the correct related note directly, or use `query <terms> --limit 10`
for broader discovery.

### JSON Parsing for AI Agents

All commands support `--json`. Use Python for structured extraction:
```python
import json, sys, subprocess
result = subprocess.run(
    ["target/debug/pkms", "--db", "~/Documents/org", "--json", "<command>", ...],
    capture_output=True, text=True
)
data = json.loads(result.stdout)
```

### Performance Notes

All subcommands are very fast even on huge notes databases.
