# pkms validate

Validate one note, or a stream of notes from NDJSON stdin.

```bash
pkms validate <target>
pkms query "topic" --output-format ndjson | pkms validate --from-stdin
pkms --output-format json validate <target>
```

`<target>` can be a UUID, file path, or note title. Validation checks metadata,
links, heading UUIDs, file links, backlinks, and task issues relevant to the
note.

Use `validate` immediately after editing a note, then run focused `check`
commands for batch-level risks.
