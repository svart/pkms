# JSON and NDJSON Output

Every command supports structured output:

```bash
pkms --output-format json check
pkms --output-format json query "rust"
pkms --output-format json stats
pkms --output-format ndjson resolve --title "graph"
```

Use JSON when one command result should be consumed as a whole. Use NDJSON when
passing a stream of note or task records to another command.

## Example JSON

```json
{
  "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "title": "Example Note",
  "path": "/home/user/Documents/org/roam/example.org"
}
```

## Example NDJSON

```json
{"uuid":"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa","title":"Example Note"}
{"uuid":"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb","title":"Related Note"}
```

NDJSON is one JSON object per line. Pipeline consumers read the `uuid` field by
default; `show` can also read task-style target records.
