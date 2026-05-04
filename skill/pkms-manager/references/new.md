# pkms new — Generate Filename and UUID for New Note

## When to Use

Use `new` when you need to create a new org-roam note. It generates a UUID v4, a timestamped filename (`YYYYMMDDHHMMSS-slug.org`), and optionally writes the boilerplate file to disk.

## How It Works

1. Generates a UUID v4
2. Converts the title to a slug (lowercase, `[a-z0-9_-]` chars, other chars → `-`)
3. Creates a timestamp in `YYYYMMDDHHMMSS` format
4. Combines into filename: `YYYYMMDDHHMMSS-slug.org`
5. Resolves the output directory (from config `new_notes_dir` or defaults to `db_root/roam`)
6. Without `--create`: dry-run — only prints the generated values
7. With `--create`: writes the boilerplate `.org` file with `:ID:` property, `#+title:`, optional filetags and aliases

## Arguments & Flags

| Arg/Flag | Default | Description |
|----------|---------|-------------|
| `title` | required | Title of the new note |
| `--create` | false | Actually write the boilerplate file |
| `--tags T1,T2` | — | Comma-separated filetags |
| `--aliases A1,A2` | — | Comma-separated aliases |

## Boilerplate File Format

With `--tags learning,emacs --aliases "Alt Name"`:

```org
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: My New Note
#+filetags: :learning:emacs:
:PROPERTIES:
:ROAM_ALIASES: Alt Name
:END:
```

## Output

### Text — dry-run

```
New note:
  Title:    My New Note
  UUID:     aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
  Filename: 20240501120000-my_new_note.org
  Path:     /home/user/Documents/org/roam/my_new_note.org
  Status:   dry-run (use --create to write)
```

### Text — created

```
New note:
  Title:    My New Note
  ...
  Status:   created
```

### JSON

```json
{
  "uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "filename": "20240501120000-my_new_note.org",
  "path": "/home/user/Documents/org/roam/my_new_note.org",
  "title": "My New Note",
  "created": true
}
```

## Typical Scenarios

### Preview what would be created
```bash
pkms new "New Concept"
```

### Create a simple note
```bash
pkms new "New Concept" --create
```

### Create with tags and aliases
```bash
pkms new "New Concept" --create --tags "learning,emacs" --aliases "Alt Name,Alternate"
```
