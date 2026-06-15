# pkms query

Broad search across titles, aliases, refs, tags, and content.

```bash
pkms query "terms"
pkms query "terms" --limit 10
pkms query "terms" --title
pkms query "terms" --tags
pkms query "terms" --content
pkms query "terms" --todos
```

Use `resolve` first for exact title/alias lookup. Use `query` when the term may
appear in note content or when you need broader discovery.

`query --output-format ndjson` is a pipeline producer.
