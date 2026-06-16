# pkms check

Use `check` for whole-database health. It loads the graph and reports issues.
Exit code is nonzero when health issues are found.

## Focused Checks

```bash
pkms check
pkms check --stats
pkms check --id-links
pkms check --file-links
pkms check --attachment-links
pkms check --filetags
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
```

When no section flags are given, all default checks are shown. When section
flags are given, output is limited to those sections.

`check --stats` reports orphan counts with the same policy as default
`pkms orphans`: daily notes are excluded.

## Agent Workflow

- Use `--id-links` before fixing broken UUIDs.
- Use `--self-links` after broad UUID replacement.
- Use `--filetags` after changing note headers.
- Use JSON for automation: `pkms --output-format json check --id-links`.
