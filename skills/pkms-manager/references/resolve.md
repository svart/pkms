# pkms resolve

Fast metadata lookup without a full graph load.

```bash
pkms resolve --title <term>
pkms resolve --uuid <uuid-fragment>
pkms resolve --tags "tag1,tag2"
pkms resolve --title <term> --limit 20
pkms resolve --uuid <uuid> --fields uuid,title,path
pkms resolve --todos
```

Use `resolve` when you need UUIDs or paths before running commands that require
exact targets. `--uuid` can find note-level and heading-level IDs; heading
matches include `matched_heading_uuid` in structured output.

`resolve --output-format ndjson` is a pipeline producer.
