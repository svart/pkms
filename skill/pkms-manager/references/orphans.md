# pkms orphans — List Orphan Notes

## When to Use

Run `orphans` to find notes that have no connections to the rest of the graph — no outgoing internal links and no backlinks. These notes are isolated and may need to be linked to relevant topics.

## How It Works

`orphans` loads the full graph, then filters for nodes where both outgoing internal links and incoming backlinks are empty. A note with only `file:` or `url:` links (but no `id:` links) and no backlinks is still considered an orphan.

## Output

### Text (default)

```
Orphan notes (3):
  Orphan Note (dddddddd-dddd-4ddd-dddd-dddddddddddd)
  Another Orphan (eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee)
```

### JSON

```json
{
  "count": 3,
  "orphans": [
    {"uuid": "dddddddd-dddd-4ddd-dddd-dddddddddddd", "title": "Orphan Note", "path": "/org/roam/personal/orphan.org", "filetags": []}
  ]
}
```

### NDJSON

One JSON object per orphan:
```json
{"uuid": "...", "title": "Orphan Note", "path": "...", "filetags": []}
```

## Typical Scenarios

### Find notes needing connections
```bash
pkms orphans
```

### With machine-readable output for scripting
```bash
pkms --output-format json orphans
```

## Workflow

See "Linking Orphans to the Graph" in SKILL.md for the full workflow on integrating orphan notes.
