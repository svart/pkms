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
- **Heading backlinks** — internal links targeting heading-level UUIDs within another note

It returns exit code **0** if healthy, **1** if issues are found.

## Section flags

Each output section is controlled by its own flag. When **no flags** are given,
all sections are shown. When **specific flags** are given, only those sections
are shown.

| Flag | Section shown |
|------|---------------|
| `--stats` | Database statistics: notes, links, orphans, broken counts, parse errors, skipped, duplicates, missing titles |
| `--id-links` | Broken internal links list + duplicate UUIDs/titles + missing titles + failed files |
| `--file-links` | Broken `file:` links (count + list) |
| `--attachment-links` | Broken `attachment:` links (count + list) |
| `--filetags` | Filetags format issues |
| `--heading-backlinks` | Heading-level backlinks list |

## Output

### Text (default)

With no flags:
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
  Broken files:   0
  Broken attach:  0
  Filetags issues: 2

Duplicate UUIDs:
  <uuid> -> /path/to/note.org
  <uuid> -> /path/to/note.org

Duplicate titles:
  "Title" -> /path/to/a.org

Missing #+title:
  /path/to/no_title.org

Broken links:
  Some Note -> ffffffff-ffff-4fff-ffff-ffffffffffff

Invalid filetags format:
  Note (path): tag ':  bad:' — reason

Status: issues found
```

With `--stats` only:
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

Status: issues found
```

### JSON

With no flags (all sections present):
```json
{
  "db_root": "/home/user/Documents/org",
  "stats": { ... },
  "duplicates": { ... },
  "broken_links": [...],
  "broken_file_links": [...],
  "broken_attachment_links": [...],
  "failed_files": [...],
  "filetags_issues": [...],
  "heading_backlinks": [...],
  "healthy": false
}
```

With `--file-links` only (other sections omitted):
```json
{
  "db_root": "/home/user/Documents/org",
  "broken_file_links": [...],
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

### Check just stats (quick overview)
```bash
pkms check --stats
```

### Machine-readable output for automation
```bash
pkms --output-format json check
```
