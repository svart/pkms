# pkms orphans

List notes with no incoming or outgoing internal links.

```bash
pkms orphans
pkms orphans --with-dailies
pkms orphans --limit 20
pkms orphans --output-format ndjson
```

Use `orphans` to find isolated notes that may need real contextual links. Before
editing, inspect the orphan content and candidate related notes. Do not add
links mechanically just to remove orphan status.
