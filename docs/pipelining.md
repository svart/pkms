# Pipelining

Commands can be chained through NDJSON. Producers emit one JSON object per line,
usually with a `uuid` field. Consumers read targets from stdin when piped or
when `--from-stdin` is set.

Only the producer commands below emit record streams. Other commands that accept
`--output-format ndjson` emit one compact JSON object on one line.

## Producers

- `resolve --output-format ndjson`
- `query --output-format ndjson`
- `orphans --output-format ndjson`
- `stats --hubs --output-format ndjson`
- `suggest --output-format ndjson`
- `mentions --output-format ndjson` (`uuid` is the mentioned note)
- `rag search --output-format ndjson`
- `rag retrieve --output-format ndjson`

## Consumers

- `get`
- `suggest`
- `validate`
- `task list`

## Examples

```bash
pkms query "distributed systems" --output-format ndjson | pkms get --links
pkms resolve --tags "ai" --output-format ndjson | pkms get --links
pkms stats --hubs --output-format ndjson | pkms get --links --no-content
pkms query "concurrency" --output-format ndjson | pkms suggest --output-format ndjson | pkms get --links
pkms query "foo" --output-format ndjson | pkms validate
pkms mentions <uuid> --output-format ndjson | pkms get --links --no-content
pkms resolve --tags "project" --output-format ndjson | pkms task list --from-stdin
pkms rag retrieve "distributed mesh" --output-format ndjson | pkms task list --from-stdin
```

Use `--from-stdin` explicitly when a consumer also receives flags that make
intent ambiguous.
