# pkms context

Build an LLM-friendly context window around one or more notes.

```bash
pkms context <target> --depth 1
pkms context <target> --depth 2 --max-tokens 4000
pkms context <target> --encoding o200k_base
pkms query "topic" --output-format ndjson | pkms context --depth 1 --from-stdin
```

Use `context` when the result is meant to be sent to an LLM. Use `get` for
human inspection and smaller note-level exploration.
