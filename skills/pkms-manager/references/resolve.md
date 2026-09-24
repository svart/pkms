# pkms resolve

Fast metadata lookup without a full graph load.

```bash
pkms resolve --title <term>
pkms resolve --title <term> --exact
pkms resolve --title <term> --word
pkms resolve --title <term1> --title <term2> --output-format ndjson
pkms resolve --uuid <uuid-fragment>
pkms resolve --tags "tag1,tag2"
pkms resolve --title <term> --limit 20
pkms resolve --uuid <uuid> --fields uuid,title,path
pkms resolve --title <term> --todos
```

Use `resolve` when you need UUIDs or paths before running commands that require
exact targets.

`--title` matches when every query word appears in the title or an alias,
case-insensitively, including inside longer words: `graph` matches both
`Graph Theory` and `Paragraph`. `--word` requires whole-word matches, so
`graph` no longer matches `Paragraph`; `--exact` keeps only titles or aliases
equal to the query. Each title result
carries `match_kind` (`exact`, `alias`, `word`, or `substring`) and
`matched_query`; results are grouped per query in argument order and ranked by
match kind. Repeat `--title` to resolve many terms in one call, for example
when checking candidate link targets; `--limit` then applies per query.
`--fields` keeps only listed keys, so add `match_kind,matched_query` when you
need them with a field subset. `--todos` restricts an existing `--title`, `--uuid`, or `--tags`
lookup to notes with TODO headings. `--uuid` can find note-level and
heading-level IDs; heading matches include `matched_heading_uuid` in structured
output.

`resolve --output-format ndjson` is a pipeline producer.

Shared scope flags (`--include-tags`, `--exclude-tags`, `--path-prefix`, daily
mode, and `--modified-since`) filter matches before `--limit`.
