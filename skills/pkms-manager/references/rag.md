# RAG Retrieval and Tag Recommendations

`pkms tags` works in every build. The `pkms rag` namespace and
`pkms tags suggest` require a build with the `rag` feature. Build the index
before searching, retrieving, or suggesting tags:

```bash
pkms rag index
pkms rag status --output-format json
```

List direct note and heading tag assignments with their usage counts:

```bash
pkms tags
```

The result is sorted by usage count descending and then by tag name. JSON
matches `schemas/tags.json`; NDJSON emits one `{tag,count}` object per line.

Recommend existing corpus tags for a note or canonical local task:

```bash
pkms tags suggest <full-note-uuid>
pkms tags suggest <canonical-id>
```

Both commands preview by default. Inspect the evidence and use `--apply` only
when the recommendations fit the target:

```bash
pkms tags suggest 11111111-1111-4111-8111-111111111111 --apply
pkms tags suggest 12 --limit 3 --neighbors 30 --apply
```

`rag search`, `rag retrieve`, and `tags suggest` accept shared exact-tag,
path-prefix, daily-mode, and source modification-time filters. Use
`--without-dailies` when daily notes would dominate retrieval evidence.

Recommendations come only from tags already present in similar indexed notes
or headings. Note recommendations use filetags; task recommendations use
heading-specific tags and do not copy inherited note filetags onto the task.
Applying is additive and never removes existing tags.

JSON returns one response matching `schemas/tags-suggest.json`. NDJSON emits one
record per suggestion under the `suggestion` field. An empty recommendation set
therefore produces no NDJSON records; use JSON when the empty response itself
must be observed.

Applying changes authoritative org source only. Run `pkms rag index` explicitly
when the derived index should reflect the new tags.
