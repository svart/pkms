# JSON and NDJSON Output

Most commands accept structured output flags:

```bash
pkms --output-format json check
pkms --output-format json query "rust"
pkms --output-format json stats
pkms --output-format ndjson resolve --title "graph"
```

Use JSON when one command result should be consumed as a whole. Use NDJSON when
passing a stream of note or task records to another command.

`pkms rag index` is a text-only exception: it prints rebuild progress to stderr,
prints its final summary to stdout, and rejects `--output-format`.

NDJSON is stream-oriented only for commands that produce multiple records, such
as the producers listed in [Pipelining](pipelining.md). Commands that are not
stream producers print one compact JSON object on one line when the selected
format is `ndjson`; prefer `json` for those commands when pretty output is more
useful.

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

## Check Output Notes

`pkms --output-format json check --file-links` reports missing local or remote
files in `broken_file_links`. When SSH remote file-link checking is enabled
with `check --remote-file-links`, non-missing remote failures are reported in
`file_link_errors` instead:

```json
{
  "source_uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "source_title": "Example Note",
  "target_path": "/ssh:example.org:/srv/doc.org",
  "backend": "ssh",
  "error_kind": "hostkey",
  "message": "host example.org:22 is not present in /home/user/.ssh/known_hosts"
}
```

`file_link_errors` makes `healthy` false. Missing remote files still use
`broken_file_links`.
