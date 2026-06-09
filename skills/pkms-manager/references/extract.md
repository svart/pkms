# pkms extract

Extract a heading subtree into a new note.

```bash
pkms extract <heading-full-uuid>
pkms extract <heading-full-uuid> "New Note Title" --apply
```

`extract` is a dry run unless `--apply` is present. The UUID must belong to a
heading, not a note. The extracted heading UUID becomes the new note primary
`:ID:`.

When a title is provided, it changes only the new note `#+title`. The copied
root heading keeps its original heading text, child heading IDs remain in the
copied subtree, and the source subtree is replaced by a same-level heading link
whose label is the original heading title.
