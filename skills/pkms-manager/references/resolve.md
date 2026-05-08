# pkms resolve — Fast UUID/Title Resolution

## When to Use

Use `resolve` for quick lookups when you know a title, UUID fragment, or tag but need the exact UUID, path, or other metadata. It is the fastest command because it only reads file headers (first 100 lines) instead of parsing the full graph.

## How It Works

`resolve` scans all `.org` files in the database root, reading only the first 100 lines of each file to extract UUID, title, filetags, and aliases. When `--uuid` is specified, the full file is scanned to find both note-level and heading-level `:ID:` properties.

Results are matched via substring comparison.

## Flags

| Flag | Description |
|------|-------------|
| `--uuid <UUID>` | Search by UUID (substring match on the `:ID:` value) |
| `--title <TERM>` | Search by title or alias (all words must match, substring) |
| `--tags TAG1,TAG2` | Filter notes that have any of these filetags (substring) |
| `--limit N` | Maximum results (default: 30) |
| `--fields F1,F2` | Comma-separated fields to display: uuid,title,path,tags,aliases |

At least one of `--uuid`, `--title`, or `--tags` must be provided.

## Output

### Text (default)

```
Total: 3
  Note A
         UUID: aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Note B
         UUID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
         Tags: learning, emacs
```

With `--fields uuid,title`:
```
  Note A
         UUID: aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
```

### JSON

```json
{
  "query": "",
  "total": 3,
  "results": [
    {"uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa", "title": "Note A", "path": "...", "filetags": [], "aliases": []}
  ]
}
```

### NDJSON (with --fields)

```json
{"uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa", "title": "Note A"}
```

## Typical Scenarios

### Find a note by title
```bash
pkms resolve --title "Concept"
```

### Find a note by UUID prefix
```bash
pkms resolve --uuid "aaaaaaa"
```

### Find a note by heading-level UUID
```bash
pkms resolve --uuid "ba20ac20"
```
`--uuid` matches both note-level and heading-level IDs. The output includes the note's primary UUID and optionally `matched_heading_uuid` when the match came from a heading.

### Filter by tag
```bash
pkms resolve --tags "emacs"
```

### Find replacement UUID for broken link
```bash
pkms resolve --title "Original Concept Title"
```

### Just get UUID and path
```bash
pkms resolve --title "Note" --fields uuid,path
```

### Machine-readable
```bash
pkms --output-format json resolve --title "Concept"
```
