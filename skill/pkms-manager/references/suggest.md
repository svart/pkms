# pkms suggest — Find Related Notes by Multi-Factor Scoring

## When to Use

Use `suggest` when you have a specific note and want to discover other notes that are thematically related. This is useful for finding connection opportunities, discovering related topics you may have forgotten about, and building out your knowledge graph.

**Important**: `suggest` accepts a full UUID only (not title or path). Use `resolve` first to find the UUID if needed.

## How It Works

`suggest` loads the full graph, then scores every other note against the target using seven factors:

| Factor | Weight | Description |
|--------|--------|-------------|
| **title** | 20× | Title keyword overlap (words longer than 2 chars) |
| **content** | 5× | Content keyword overlap (words longer than 3 chars, up to 50 keywords) |
| **tags** | 25× | Shared filetags |
| **backlinks** | 15× | Shared backlinks (notes that link to both) |
| **outgoing** | 12× | Shared outgoing links (notes both link to) |
| **directory** | 5 | Same parent directory (only if score > 0) |
| **neighborhood** | multiplier | Average neighbor relevance, clamped to [-0.5, 1.0] multiplier |

The target note is looked up by **exact UUID match** in the graph's node map. The target note itself is excluded from results.

## Arguments & Flags

| Arg/Flag | Default | Description |
|----------|---------|-------------|
| `target` | required | Exact UUID of the target note |
| `-l`, `--limit` | 10 | Number of suggestions to return |
| `--exclude-orphans` | false | Exclude orphan notes from suggestions |

## Output

### Text (default)

```
Suggestions for "Note A":

  1. Note B  (score: 120.0)
       UUID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
       Matches: backlinks, tags, title
  2. Note C  (score: 85.0)
       UUID: cccccccc-cccc-4ccc-cccc-cccccccccccc
       Matches: content, outgoing
```

### JSON

```json
{
  "target": "Note A",
  "target_uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "suggestions": [
    {
      "uuid": "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
      "title": "Note B",
      "path": "/org/roam/common/note_b.org",
      "score": 120.0,
      "reasons": ["shared tags", "2 shared backlinks"],
      "filetags": ["learning"],
      "scores": {"title": 20.0, "tags": 25.0, "backlinks": 30.0, "neighborhood": 0.15}
    }
  ]
}
```

## Typical Scenarios

### Find related notes for a hub
```bash
pkms suggest "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
```

### Top 5 most relevant with no orphans
```bash
pkms suggest "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa" --limit 5 --exclude-orphans
```

### Machine-readable with factor breakdowns
```bash
pkms --output-format json suggest "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"
```
