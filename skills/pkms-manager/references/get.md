# pkms get

Retrieve note content, headings, and immediate graph neighbors.

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --headings
pkms get <target> --links --no-content
pkms get <target> --encoding o200k_base
pkms query "topic" --output-format ndjson | pkms get --links --from-stdin
```

Targets can be UUIDs, paths, or note titles. Use `--headings --no-content` to
find heading UUIDs without loading the full note body into the response.
