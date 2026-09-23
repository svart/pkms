# pkms new

Generate UUIDs and note filenames, and optionally write boilerplate notes.

```bash
pkms new "Title"
pkms new "Title" --create
pkms new "Title" --create --tags "tag1,tag2"
pkms new "Title" --create --aliases "Alias One,Alias Two"
pkms new "Title" --create --body draft.org
printf '* Section\nText\n' | pkms new "Title" --create --body -
pkms new "Existing Note" --create --heading "Heading Title"
```

Without `--create`, `new` is a dry run. `--heading` generates a heading-level
`:ID:` for an existing heading in the note. `--body <PATH|->` appends org
content from a file or stdin (`-`) after the generated header; it requires
`--create`, conflicts with `--heading`, and the body must not repeat the
`:PROPERTIES:`/`#+title` header.

After creation or heading-ID changes, run:

```bash
pkms validate <new-or-changed-note>
```
