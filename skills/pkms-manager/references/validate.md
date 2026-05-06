# pkms validate — Single Note Health Check

## When to Use

After creating or editing a single note, run `validate` to confirm the note has no issues before running a full `check`. Use during link-fixing workflows to verify each changed note individually.

## How It Works

`validate` loads the full graph, resolves the target note by UUID, file path, or title, then inspects:

- **UUID format** — ensures the `:ID:` property is a valid 5-part UUID v4
- **Title presence** — ensures `#+title:` exists in the file
- **Broken internal links** — outgoing `id:` links pointing to non-existent UUIDs
- **Broken file links** — outgoing `file:` links whose target does not exist on disk
- **Backlinks** — all notes that link to this one

## Arguments

| Arg | Description |
|-----|-------------|
| `target` | UUID, file path, or note title of the note to validate |

## Output

### Text (default)

```
Note: My Note
  UUID:   aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Path:   /org/roam/common/note.org
  Tags:   learning, emacs
  Aliases: Alias One
  Headings: 3

Links:
  Outgoing: 5 (4 internal)
  Incoming: 12
  Broken:   1 internal, 0 file

Broken internal links:
  -> ffffffff-ffff-4fff-ffff-ffffffffffff

Status: 1 issue(s)
```

### JSON

```json
{
  "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "title": "My Note",
  "path": "/org/roam/common/note.org",
  "filetags": ["learning", "emacs"],
  "aliases": ["Alias One"],
  "refs": [],
  "headings": 3,
  "outgoing": 5,
  "incoming": 12,
  "outgoing_internal": 4,
  "broken_internal": ["ffffffff-ffff-4fff-ffff-ffffffffffff"],
  "broken_files": [],
  "backlinks": [{"uuid": "...", "title": "Source Note"}],
  "issues": ["1 broken internal link(s)"],
  "healthy": false
}
```

## Typical Scenarios

### Validate a note by title
```bash
pkms validate "My Note"
```

### Validate after editing
Always run after creating or editing a note:
```bash
pkms validate "My Note"
pkms check
```

### Machine-readable output
```bash
pkms --output-format json validate "My Note"
```
