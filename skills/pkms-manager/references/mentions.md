# pkms mentions

Find unlinked references: phrases that name a note by title or alias but are
not links. One scan replaces resolving candidate terms one by one.

```bash
pkms mentions <uuid-or-title> --output-format ndjson       # names in this note
pkms mentions <uuid-or-title> --incoming --output-format json  # notes naming this one
pkms mentions - --output-format ndjson < draft.org          # a draft before it exists
pkms mentions <uuid> --headings --min-length 2              # widen candidates
```

Record fields:

- `line`, `col`: 1-based position in the source file; `col` counts characters.
  For a heading target, `line` still refers to the whole file.
- `phrase`: matched text as written.
- `uuid`, `title`: the mentioned note. This is what `| pkms get` consumes.
- `match`: `title` or `alias`.
- `already_linked`: the scanned text already has an `id:` link to `uuid`.
- `source_uuid`, `source_title`, `path`: the scanned note; `null` for stdin.

JSON wraps records as `{target, target_uuid, mode, count, mentions}`.

Behavior:

- The target is a UUID, title, alias, or path. `-` reads org text from stdin;
  there is no implicit stdin detection. `--incoming` cannot read stdin.
- A heading UUID target scans only that heading's subtree.
- The command never reports text inside links, URLs, keyword lines, drawers,
  planning lines, src/example/export blocks, `~code~`, `=verbatim=`,
  timestamps, or heading tags.
- The default mode skips names shorter than `--min-length` (default 3), daily
  notes, and heading nodes unless `--headings` is set. Scope flags limit the
  mentioned notes.
- `--incoming` includes daily notes as sources. Scope flags limit the source
  notes.
- A note never reports itself.

Every record is a candidate, not an instruction to link. Before adding a link,
confirm that the sentence is about the mentioned concept, pick the first
justified occurrence, and skip records where `already_linked` is true unless
the existing link is in the wrong place.
