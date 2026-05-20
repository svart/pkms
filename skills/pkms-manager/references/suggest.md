# pkms suggest

Find notes related to one or more target UUIDs.

```bash
pkms suggest <uuid>
pkms suggest <uuid> --limit 5
pkms suggest <uuid> --exclude-orphans
pkms query "topic" --output-format ndjson | pkms suggest --limit 3 --from-stdin
```

`suggest` accepts UUID targets. Use `resolve` first when starting from a title.

When built with the `embed` feature:

```bash
pkms suggest <uuid> --embed
```

Use suggestions as candidates, not proof. Read candidate notes before adding
links.
