# pkms new

Generate UUIDs and note filenames, and optionally write boilerplate notes.

```bash
pkms new "Title"
pkms new "Title" --create
pkms new "Title" --create --tags "tag1,tag2"
pkms new "Title" --create --aliases "Alias One,Alias Two"
pkms new "Existing Note" --create --heading "Heading Title"
```

Without `--create`, `new` is a dry run. `--heading` generates a heading-level
`:ID:` for an existing heading in the note.

After creation or heading-ID changes, run:

```bash
pkms validate <new-or-changed-note>
```
