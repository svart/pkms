# pkms orphans

List notes with no incoming or outgoing internal links.

```bash
pkms orphans
pkms orphans --with-dailies
pkms orphans --limit 20
pkms orphans --output-format ndjson
```

By default, daily notes are excluded from orphan detection. Use
`--with-dailies` only when explicitly auditing daily notes or debugging orphan
count differences.

Tag, path-prefix, and modification-time scope filters run before the orphan
count and limit. `--without-dailies` states the default explicitly and conflicts
with `--with-dailies`.

Use `orphans` to find isolated notes that may need real contextual links. Before
editing, inspect the orphan content and candidate related notes. Do not add
links mechanically just to remove orphan status.
