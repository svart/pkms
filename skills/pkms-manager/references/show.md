# pkms show

Show detailed information for a task heading.

```bash
pkms show <canonical-id>
pkms show <uuid-or-title>
pkms show --uuid <uuid-or-title>
pkms task list --output-format ndjson | pkms show --from-stdin
pkms --output-format json show <canonical-id>
```

Numeric targets are treated as canonical task IDs. Use `--uuid` when a
numeric-looking value should be resolved as a note target instead.

The output includes note metadata, heading metadata, parent and child tasks,
outgoing links, and the heading block content.
