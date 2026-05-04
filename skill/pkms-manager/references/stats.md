# pkms stats — Comprehensive Database Statistics

## When to Use

Use `stats` to get an overview of the database: total notes, link counts, orphan rate, disk usage, and directory breakdown. Use `--hubs` to find the most-connected notes (good starting points for exploration). Use `--tags` to browse all filetags with counts.

## How It Works

`stats` loads the full graph and computes aggregate metrics across all notes.

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--days N` | — | Include recently modified notes (last N days) in output |
| `--hubs [N]` | 10 | Show top N most-connected notes by degree (outgoing + incoming internal links) |
| `--tags` | — | List all unique filetags with note counts |

## Output

### Text (default)

```
Database: /home/user/Documents/org
  Notes:             1450
  Links:             12450 (avg: 8.59/note)
    Internal:        9800
    File:            450
    URL:             2200
  Orphans:           42
  Broken links:      3
  Disk size:         42.5 MB
```

### JSON

```json
{
  "db_root": "/home/user/Documents/org",
  "total_notes": 1450,
  "total_links": 12450,
  "internal_links": 9800,
  "file_links": 450,
  "url_links": 2200,
  "avg_links_per_note": 8.59,
  "orphans": 42,
  "broken_links": 3,
  "disk_size_bytes": 44564480,
  "directories": [{"directory": "roam/common", "count": 681}, ...],
  "recent_notes": null
}
```

### Hubs output (text)

```
Top 10 hubs:
   1. Central Concept                          245 links (120 out / 125 in)  a1b2c3d4
   2. Another Hub                              180 links (90 out / 90 in)   e5f6a7b8
```

### Hubs output (JSON)

```json
{
  "limit": 10,
  "hubs": [{"rank": 1, "uuid": "...", "title": "Central Concept", "degree": 245, "outgoing": 120, "incoming": 125}]
}
```

### Tags output

```
Filetags (count):
  learning                      142
  emacs                         89
  ...
```

### Tags output (JSON)

```json
{"tags": [{"tag": "learning", "count": 142, "notes": []}]}
```

## Typical Scenarios

### Database overview
```bash
pkms stats
```

### Find exploration starting points
```bash
pkms stats --hubs
```

### Browse by tags
```bash
pkms stats --tags
```

### Check recent activity
```bash
pkms stats --days 30
```

### Machine-readable for analysis
```bash
pkms --output-format json stats
```
