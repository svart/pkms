# pkms suggest

Find notes related to one or more target UUIDs.

```bash
pkms suggest <uuid>
pkms suggest <uuid> --limit 5
pkms suggest <uuid> --all
pkms suggest <uuid> --exclude-orphans
pkms query "topic" --output-format ndjson | pkms suggest --limit 3 --from-stdin
```

`suggest` accepts UUID targets and returns at most ten candidates per target by
default. Use `--limit N` for another positive bound or `--all` explicitly for
every candidate. Use `resolve` first when starting from a title.

Use suggestions as candidates, not proof. Read candidate notes before adding
links.
