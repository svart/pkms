# pkms

A CLI tool for navigating, managing, and validating org-roam personal knowledge management systems.

## Overview

`pkms` helps AI agents and users interact with an org-roam database of interconnected notes. It provides commands for database health checks, note retrieval with graph traversal, fuzzy search, new note creation, and link validation.

## Quick Start

```bash
# Check health of the entire database (exit code 1 if issues found)
pkms --db ~/Documents/org check

# Validate a specific note with full health report
pkms --db ~/Documents/org validate <uuid-or-path>

# Generate filename and UUID for a new note
pkms --db ~/Documents/org new "My Note Title"

# Retrieve a note with its neighbors at depth 2
pkms --db ~/Documents/org get <uuid> --depth 2

# Visual ASCII graph of a note's neighborhood
pkms --db ~/Documents/org get "note title" --depth 1 --graph

# Fuzzy search across notes
pkms --db ~/Documents/org query "search terms"

# JSON output for AI consumption
pkms --db ~/Documents/org --json query "rust"

# Find shortest path between two notes
pkms --db ~/Documents/org path "note A" "note B"

# Export subgraph around a note
pkms --db ~/Documents/org subgraph <uuid> --depth 2

# List all filetags with note counts
pkms --db ~/Documents/org tags

# List notes with a specific tag
pkms --db ~/Documents/org tags --tag book

# Add a link from one note to another
pkms --db ~/Documents/org add-link <source> <target>

# Show current configuration
pkms info
```

## Commands

| Command       | Description                                      |
|---------------|--------------------------------------------------|
| `check`       | Verify health of the entire org-roam database     |
| `validate`    | Validate health of a specific note                |
| `get`         | Retrieve a note with neighbors at depth N         |
| `query`       | Fuzzy search across note titles and content       |
| `path`        | Find shortest path between two notes              |
| `subgraph`    | Export subgraph around a note with stats          |
| `tags`        | List all filetags with note counts                |
| `new`         | Generate a filename and UUID for a new note       |
| `add-link`    | Add a link from one note to another               |
| `info`        | Show current pkms configuration                   |
| `init-config` | Generate default config file                      |

## Global Flags

| Flag         | Description                                         |
|--------------|-----------------------------------------------------|
| `--db`       | Path to org-roam database root (overrides config)   |
| `--json`     | Structured JSON output for AI/script consumption    |

## Configuration

The tool reads `~/.config/pkms.toml` for persistent settings:

```toml
# Path to the org-roam database root
db_root = "/home/user/Documents/org"

# Directory where new notes should be created (relative to db_root or absolute)
new_notes_dir = "roam"

# Glob patterns to ignore during file discovery
ignore_patterns = [".attach", ".git", "*.bak"]
```

The `--db` CLI flag overrides the `db_root` from config. If no config file and no `--db` are provided, the tool errors with instructions.

## Database Format

The tool expects an [org-roam](https://www.orgroam.com/) directory with:
- Notes named `YYYYMMDDHHMMSS-slug.org` with UUID v4 `:ID:` properties
- Internal links using `[[id:<uuid>][description]]` format
- Subdirectories: `roam/`, `roam/common/`, `roam/personal/`, `roam/biblio/`

See [TODO.md](./TODO.md) for the full implementation roadmap.
