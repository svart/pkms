# pkms query

Broad search across titles, aliases, refs, tags, and content.

```bash
pkms query "terms"
pkms query "terms" --limit 10
pkms query "terms" --title
pkms query "terms" --tags
pkms query "terms" --content
pkms query "terms" --todos
pkms query "terms" --max-matches-per-note 5
pkms query "terms" --all-matches
```

Use `resolve` first for exact title/alias lookup. Use `query` when the term may
appear in note content or when you need broader discovery.

Each result includes at most three content matches by default and reports the
uncapped count as `content_matches_total`. Use `--max-matches-per-note N` for a
different positive bound or `--all-matches` for the complete match list.

Use shared `--include-tags`, `--exclude-tags`, `--path-prefix`,
`--with-dailies`/`--without-dailies`, and `--modified-since` flags to constrain
the note scope before result counts and limits are applied.

`query --output-format ndjson` is a pipeline producer.
