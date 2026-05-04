# pkms context — Build AI Context Window

## When to Use

Use `context` when you need to feed a note and its surrounding graph to an AI (LLM). It produces a single formatted text block containing the target note's full content plus linked neighbors at each depth level, with an estimated token count so you can stay within model limits.

## How It Works

1. Resolves the target note (UUID, file path, or title)
2. Loads the note's full file content
3. Traverses outgoing and incoming links up to `--depth` levels
4. Renders everything through a template with sections for tags, aliases, content, forward links, and backlinks
5. Optionally truncates to fit within `--max-tokens`

## Arguments & Flags

| Arg/Flag | Default | Description |
|----------|---------|-------------|
| `target` | required | UUID, file path, or note title |
| `-d`, `--depth` | 1 | Traversal depth for neighbor expansion |
| `-m`, `--max-tokens` | unlimited | Maximum token budget for truncation |

## Output

### Text (default)

The rendered context is printed to stdout. Token estimate and depth info go to stderr.

```
# Note Title
UUID: aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
Path: /org/roam/common/note.org

--- Content ---
Full note content here...
--- End Content ---

=== Depth 1 ===
Forward links:

  → Linked Note (bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb)
    Path: /org/roam/common/linked.org
    [truncated content]

=== Depth 1 ===
Backlinks:

  ← Source Note (cccccccc-cccc-4ccc-cccc-cccccccccccc)
    Path: /org/roam/common/source.org
```

Stderr:
```
[context: ~450 tokens, depth: 2, max_tokens: 2000]
```

### JSON

```json
{
  "target": "Note Title",
  "context": "# Note Title\nUUID: ...\n...",
  "estimated_tokens": 450,
  "depth": 2
}
```

## Typical Scenarios

### Quick context for LLM
```bash
pkms context "Note Title" --depth 1
```

### Deep exploration with budget
```bash
pkms context "Note Title" --depth 2 --max-tokens 4000
```

### Machine-readable for programmatic use
```bash
pkms --output-format json context "Note Title" --depth 2
```
