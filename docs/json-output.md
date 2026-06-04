# JSON and NDJSON Output

Every command accepts structured output flags:

```bash
pkms --output-format json check
pkms --output-format json query "rust"
pkms --output-format json stats
pkms --output-format ndjson resolve --title "graph"
```

Use JSON when one command result should be consumed as a whole. Use NDJSON when
passing a stream of note or task records to another command.

NDJSON is stream-oriented only for commands that produce multiple records, such
as the producers listed in [Pipelining](pipelining.md). Commands that are not
stream producers may print a single JSON object in structured mode even when the
selected format is `ndjson`; prefer `json` for those commands.

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
default unless the command documents a narrower input shape.
