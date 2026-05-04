# pkms query — Fuzzy Search Across Note Titles and Content

## When to Use

Use `query` for broad searches across the entire database. Unlike `resolve` (which only reads file headers), `query` loads the full graph and searches note titles, aliases, refs, tags, and file content. Results are scored and sorted by relevance.

## How It Works

1. Scans and parses all `.org` files to build the full graph
2. Searches titles, aliases, and refs using substring matching — each matching field adds to the result score
3. If content search is enabled, scans all file content for matching lines (uses `rg`-style matching)
4. Merges title and content results, deduplicating by UUID
5. Sorts by descending score
6. Optionally truncates to `--limit`

Search scope flags work independently or in combination:
- No scope flags = search all (title + alias + ref + tag + content)
- Only `--tags` = search tags only
- Only `--title` = search title, alias, ref only
- Only `--content` = search file content only

## Arguments & Flags

| Arg/Flag | Description |
|----------|-------------|
| `terms` | Search terms |
| `--limit N` | Maximum results (default: unlimited) |
| `--tags` | Search only in filetags |
| `--title` | Search only in titles, aliases, and refs |
| `--content` | Search only in file content |

## Output

### Text (default)

```
Query: Concept
Results: 5

  1. Central Concept  (score: 3.0)
       UUID: aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
       Matches: title
  2. Related Idea  (score: 2.0)
       UUID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
       Matches: alias
       > This line mentions the concept in context...
```

### JSON

```json
{
  "query": "Concept",
  "total_results": 5,
  "results": [
    {
      "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
      "title": "Central Concept",
      "path": "/org/roam/common/concept.org",
      "filetags": ["learning"],
      "score": 3.0,
      "matches": ["title"],
      "content_matches": []
    }
  ]
}
```

## Typical Scenarios

### Broad search
```bash
pkms query "machine learning"
```

### Tag-only search
```bash
pkms query "emacs" --tags
```

### Title-only search
```bash
pkms query "Concept" --title
```

### Content-only with limit
```bash
pkms query "specific phrase" --content --limit 5
```

### Machine-readable
```bash
pkms --output-format json query "Concept"
```
