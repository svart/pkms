# pkms get — Retrieve Note with Neighbors

## When to Use

Use `get` when you need to see a note's full content together with its immediate neighbors (forward links and backlinks). This is useful for deep exploration of a specific topic's graph neighborhood.

## How It Works

1. Resolves the target note (UUID, file path, or title)
2. Loads the note's full file content
3. With `--links`, fetches depth-1 neighbors (outgoing notes and backlinks)
4. Without `--links`, shows only the note and its content

Content is shown by default; use `--no-content` to suppress it.

## Arguments & Flags

| Arg/Flag | Description |
|----------|-------------|
| `target` | UUID, file path, or note title |
| `--links` | Show forward links (outgoing) and backlinks at depth 1 |
| `--headings` | Show heading structure extracted from note content |
| `--no-content` | Suppress note content output |

## Output

### Text — with content but no links

```
Note: Note A
  UUID:   aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Path:   /org/roam/common/note_a.org

--- Content ---
Full file content...
--- End Content ---
```

### Text — with headings

```
Note: Note A
  UUID:   aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Path:   /org/roam/common/note_a.org

--- Headings ---
* Introduction  :tag1:
** DONE Background
* Main Section
** Subsection (aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa)
--- End Headings ---
```

### Text — with links and no content

```
Note: Note A
  UUID:   aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Path:   /org/roam/common/note_a.org

Forward links:
  Note B (bbbbbbbb)

Backlinks:
  Source Note (cccccccc)
```

### JSON

```json
{
  "node": {
    "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    "title": "Note A",
    "path": "/org/roam/common/note_a.org",
    "filetags": [],
    "categories": [],
    "content": "Full file content...",
    "headings": [
      {"level": 1, "title": "Introduction", "todo_state": null, "tags": ["tag1"], "raw": "* Introduction     :tag1:"},
      {"level": 2, "title": "Subsection", "todo_state": null, "tags": [], "raw": "** Subsection", "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"}
    ],
    "headings_count": 2
  },
  "neighbors": {
    "1": {
      "outgoing": [{"uuid": "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb", "title": "Note B", "path": "...", "filetags": []}],
      "incoming": [{"uuid": "cccccccc-cccc-4ccc-cccc-cccccccccccc", "title": "Source Note", "path": "...", "filetags": []}]
    }
  }
}
```

## Typical Scenarios

### Quick content check
```bash
pkms get "Note A"
```

### Explore connections
```bash
pkms get "Note A" --links
```

### Show heading structure
```bash
pkms get "Note A" --headings
```

### Links only
```bash
pkms get "Note A" --links --no-content
```

### Machine-readable
```bash
pkms --output-format json get "Note A" --links
```
