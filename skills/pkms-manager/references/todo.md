# pkms todo

List TODO headings using the canonical task ID space.

```bash
pkms todo
pkms todo --columns Id,Date,State,Type,Prio,Tags,Note,Heading
pkms todo --group state
pkms todo --sort file
pkms todo --state "TODO"
pkms todo --state "!DONE"
pkms todo --tags "agenda,project"
pkms todo --type "SCHED"
pkms todo --prio A
pkms todo --after 2026-01-01
pkms todo --before 2026-01-31
pkms todo --scope <uuid-or-title>
pkms resolve --tags "project" --output-format ndjson | pkms todo --from-stdin
```

The `Id` column is shared with `agenda`, `show`, and `open`. Filtered views can
show non-contiguous IDs.
