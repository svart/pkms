# pkms check — Full Database Health Scan

## When to Use

After making changes to notes (creating, editing, fixing links), always run `check` to verify the entire database is in a valid state. Use before committing changes to ensure no regressions were introduced.

## How It Works

`check` loads the full graph from disk, then analyzes every note for:

- **Broken `id:` links** — internal links where the target UUID does not exist in any note
- **Duplicate UUIDs** — two or more files sharing the same `:ID:` property
- **Duplicate titles** — two or more files sharing the same `#+title:` value
- **Missing titles** — files without a `#+title:` keyword
- **Parse errors** — files that could not be read (IO errors)
- **Skipped files** — files with no UUID property or matching ignore patterns
- **Broken `file:` links** — file links whose target path does not exist on disk
- **Broken `attachment:` links** — attachment links whose target path does not exist on disk

It returns exit code **0** if healthy, **1** if issues are found.

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| (none) | all checks | Check id:, file:, and attachment: links |
| `--file-links` | off | Check that `file:` link targets exist on disk |
| `--attachment-links` | off | Check that `attachment:` link targets exist on disk |
| `--id-links` | off | Check only that `id:` link targets exist in the database |

When no flags are given, all three link types are checked. Flags can be combined to narrow scope.

## Output

### Text (default)

```
Database: /home/user/Documents/org
  Notes:          1450
  Links:          12450 (internal: 9800, file: 450, url: 2200)
  Orphans:        42
  Broken links:   3
  Parse errors:   0
  Skipped files:  1
  Dup UUIDs:      0
  Dup titles:     1
  Missing titles: 0

Broken links:
  Some Note -> ffffffff-ffff-4fff-ffff-ffffffffffff

Status: issues found
```

### JSON

```json
{
  "db_root": "/home/user/Documents/org",
  "stats": {
    "total_notes": 1450,
    "total_links": 12450,
    "total_internal_links": 9800,
    "total_file_links": 450,
    "total_url_links": 2200,
    "orphan_notes": 42,
    "broken_link_count": 3,
    "skipped_count": 1,
    "parse_error_count": 0,
    "duplicate_uuid_count": 0,
    "duplicate_title_count": 1,
    "missing_title_count": 0
  },
  "duplicates": {
    "duplicate_uuids": [],
    "duplicate_titles": [{"value": "Duplicate Title", "paths": ["path1.org", "path2.org"]}],
    "missing_titles": []
  },
  "broken_links": [{"source_uuid": "...", "source_title": "Some Note", "target_uuid": "ffffffff-ffff-4fff-ffff-ffffffffffff"}],
  "broken_file_links": [],
  "broken_attachment_links": [],
  "failed_files": [],
  "healthy": false
}
```

## Typical Scenarios

### After editing notes
```bash
pkms check
```

### Focus on file link health only
```bash
pkms check --file-links
```

### Machine-readable output for automation
```bash
pkms --output-format json check
```
