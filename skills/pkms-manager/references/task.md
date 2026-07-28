# Local task workflows

`pkms task` manages TODO headings in the org database.

## Inspect

```bash
pkms task list
pkms task list state:TODO tags:work,!blocked prio:A
pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task calendar
pkms task inbox
pkms task list projects
pkms task list tags
```

Canonical IDs are global, deterministic positive integers. Filtered views may
have gaps because excluded tasks retain their global positions.

Useful filters:

- `state:TODO`, `state:opened`, `state:!closed`
- `tags:work,!blocked`, `tag:phone`
- `type:SCHED,DEADL`
- `prio:A,B`, `priority:none`
- `date:today`, `date:week`, `date:overdue`, `date:upcoming`
- `after:YYYY-MM-DD`, `before:YYYY-MM-DD HH:MM`
- `scope:<note-title-uuid-or-path>`
- `project:<name>`, including exclusions

Use `--output-format json` for a wrapper with `total` and `items`, or
`--output-format ndjson` for one `TaskItem` per line. The maintained schema is
`../schemas/task-item.json`.

## Columns

```bash
pkms task list --columns Id,Heading
pkms task list --columns +Project
pkms task agenda --columns -Project
pkms task list --line-sep
```

Columns are `Id`, `Date`, `State`, `Type`, `Prio`, `Tags`, `Project`, `Note`,
and `Heading`. Defaults may be global or under `[columns.pkms]`.

## Show and open

```bash
pkms task 5 show
pkms task 5 open
pkms task 5 open --editor "emacsclient -n" --line 42
```

Show output includes relevant parent and child heading chains. Open resolves
the canonical ID to the current source line.

## Create

Configure `[tasks].inbox`, or pass `note:<target>`:

```bash
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Child task"
```

Supported modifiers include `title:`, `state:`, `tag:`/`tags:`,
`sch:`/`due:`, `dead:`/`dl:`, `project:`/`proj:`, `prio:`, `desc:`, `note:`,
and `dep:`/`depend:`.

Dates accept ISO dates, optional times, time-only values for today, and
unambiguous prefixes of today, tomorrow, and weekdays.

## Mutate

```bash
pkms task 5 state WAITING
pkms task 5 state DONE --dry-run
pkms task 5 done
pkms task 5 done --dry-run
pkms task 5 mod title:"New title" sch:tomorrow
pkms task 5 mod sch:
pkms task 5 mod dl:
pkms task 5 mod dep:2
pkms task 5 mod dep:
pkms task 5 postpone
pkms task 5 postpone --to 2026-06-01
```

Use explicit modifiers with `mod`; positional title text is rejected. Empty
values clear supported properties. A no-op exits nonzero. Recurring postpone
preserves org repeater and warning syntax.

State changes use configured agenda states. Closing a parent with an open child
fails. Mutations may renumber canonical IDs; when they do, `pkms` warns on
stderr while keeping structured stdout parseable.
