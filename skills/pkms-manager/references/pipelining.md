# pkms Pipelining

Use NDJSON to chain commands. Producers emit one JSON object per line; consumers
read targets from stdin automatically when possible or explicitly with
`--from-stdin`.

## Producers

- `resolve --output-format ndjson`
- `query --output-format ndjson`
- `orphans --output-format ndjson`
- `stats --hubs --output-format ndjson`
- `suggest --output-format ndjson`

## Consumers

- `get`
- `suggest`
- `validate`
- `context`
- `todo`
- `show`

## Examples

```bash
pkms query "distributed systems" --output-format ndjson | pkms get --links
pkms resolve --tags "ai" --output-format ndjson | pkms get --links
pkms stats --hubs --output-format ndjson | pkms get --links --no-content
pkms query "concurrency" --output-format ndjson | pkms suggest --output-format ndjson | pkms get --links
pkms query "foo" --output-format ndjson | pkms validate
pkms resolve --tags "project" --output-format ndjson | pkms todo --from-stdin
pkms todo --output-format ndjson | pkms show --from-stdin
```

Use `--from-stdin` when the consumer also has flags or scope could be ambiguous.
