# TODO and Agenda

`todo`, `agenda`, `show`, and `open` share one canonical task ID space. The ID
shown by `todo` is the same ID used by `agenda`, `show <ID>`, and `open <ID>`.

IDs are assigned globally using a deterministic sort:

1. Open task states first.
2. Closed task states after open tasks.
3. Within each group, by priority, path, and line number.

IDs remain stable while the underlying files do not change. Filtered views may
show non-contiguous IDs because excluded items still keep their global IDs.

## TODO

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
pkms todo --line-sep
```

`todo` lists headings in configured open TODO states unless filters select other
states.

## Agenda

```bash
pkms agenda
pkms agenda --today
pkms agenda --week
pkms agenda --overdue
pkms agenda --upcoming
pkms agenda --date 2026-01-01
pkms agenda --columns Id,Date,State,Type,Prio,Tags,Note,Heading
pkms agenda --state "TODO"
pkms agenda --tags "!device,agenda"
pkms agenda --type "SCHED,DEADL"
pkms agenda --prio A
pkms agenda --sort "date,priority"
```

`agenda` shows headings with `SCHEDULED` or `DEADLINE` timestamps and can focus
on today, the current week, overdue items, upcoming items, or a specific date.

## Filters

Comma-separated filters use AND logic. Prefix a value with `!` for negation.

```bash
pkms todo --state "TODO"
pkms todo --state "!DONE,!CANCELLED"
pkms agenda --tags "agenda,project"
pkms agenda --tags "!private"
pkms agenda --type "SCHED,DEADL"
```

## Columns

Available table columns:

```text
Id,Date,State,Type,Prio,Tags,Note,Heading
```

Use `--line-sep` for row separator lines in text output.

## Show and Open

Inspect a task by canonical ID:

```bash
pkms show 5
```

Open a task at its source heading:

```bash
pkms open 5
```

By default, `open` runs `emacsclient -n`. Override it with `--editor` or open a
specific line with `--line`.

```bash
pkms open <uuid-or-title> --editor "emacsclient -n"
pkms open <uuid-or-title> --line 42
```

Use `show --uuid <target>` when a numeric-looking target should be treated as a
note target rather than a canonical task ID.
