# pkms stats

Use `stats` for database overview and exploration entry points.

```bash
pkms stats
pkms stats --days 30
pkms stats --hubs
pkms stats --hubs 20
pkms stats --tags
pkms stats --todos
```

`--hubs` is useful before research because hub notes often make good starting
points. `--tags` helps browse topic areas. `--todos` summarizes task state per
note.

Orphan counts in stats exclude daily notes and should match default
`pkms orphans` output. Use `pkms orphans --with-dailies` only for explicit
daily-note diagnostics.

`stats --hubs --output-format ndjson` is a pipeline producer.
