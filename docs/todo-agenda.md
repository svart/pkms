# TODO and Agenda

This page is the user-facing guide for local org-mode task commands. For
implementation rules, see [Task System Design](task-system.md).

## Canonical IDs

`task list`, `task agenda`, and task actions share one deterministic local task
ID space. IDs sort open states before closed states, then timestamped files
newest first, equal timestamps by filename, daily files as midnight, other
files by path, and headings by line number. Mutable task properties and
clock-relative concepts do not affect ID assignment.

Task IDs are positive integers such as `12`. Filtered views may show
non-contiguous IDs because excluded tasks retain their global positions.

## List and Agenda

```bash
pkms task list
pkms task list state:TODO tags:work,!blocked prio:A
pkms task list --sort date,priority --limit 20
pkms task list --group state
pkms task list --from-stdin

pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task agenda date:today,overdue
```

The agenda shortcuts are aliases for `date:today`, `date:week`,
`date:overdue`, and `date:upcoming`. A bounded upcoming window includes
overdue tasks plus the requested number of days.

`task calendar` is a compact text-only monthly view:

```bash
pkms task calendar
pkms task calendar -m 3
pkms task calendar -m -1
```

## Filters

Comma-separated values use AND logic. Prefix a value with `!` for exclusion.

| Filter | Syntax |
|---|---|
| Source compatibility | `source:pkms`, `src:pkms` |
| State | `state:TODO`, `state:opened`, `state:!closed` |
| Tags | `tags:work,!blocked`, `tag:phone` |
| Planning type | `type:SCHED,DEADL`, `kind:!SCHED` |
| Priority | `prio:A,B`, `priority:none` |
| Date | `date:today`, `date:week`, `date:overdue`, `date:upcoming`, `date:YYYY-MM-DD` |
| Range | `after:YYYY-MM-DD[ HH:MM]`, `before:YYYY-MM-DD[ HH:MM]` |
| Scope | `scope:<note-title-uuid-or-path>` |
| Project | `project:<name>`, `project:!name` |

Date filters also accept the same unambiguous words and weekday prefixes as
task modifiers, such as `tom` and `fri`.

## Columns and Output

Available text columns are:

```text
Id,Date,State,Type,Prio,Tags,Project,Note,Heading
```

`--columns Id,Heading` replaces the active set. `--columns +Project` and
`--columns -Project` adjust it. `--line-sep` adds row separators. Defaults may
be configured globally or under `[columns.pkms]`.

JSON and NDJSON task records use the stable `TaskItem` shape with `source:
"pkms"`, `source_id`, `display_id`, state, dates, note identity, path, and line
metadata. NDJSON emits one object per line.

## Inbox and Creation

Configure the local inbox:

```toml
[tasks]
inbox = "Inbox"
```

The value may be a note title, UUID, absolute path, path relative to `db_root`,
or `daily`. Daily mode writes under today’s top-level `* Inbox` heading and
uses `daily_notes_dir`, falling back to `new_notes_dir`.

```bash
pkms task inbox
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent"
```

Modifiers and aliases:

| Modifier | Aliases |
|---|---|
| `title:<text>` | |
| `state:<state>` | |
| `tag:<value>` | `tags:`, `label:`, `labels:` |
| `schedule:<date>` | `sch:`, `sched:`, `due:` |
| `deadline:<date>` | `dead:`, `dl:` |
| `project:<value>` | `proj:` |
| `prio:<A-B-C>` | `priority:`, `pri:` |
| `desc:<text>` | `description:`, `body:` |
| `note:<target>` | |
| `dep:<task-id>` | `depend:` |

Dates accept `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, `HH:MM` for today, and
unambiguous case-insensitive prefixes of today, tomorrow, or weekdays.

## Show, Open, and Mutations

```bash
pkms task 5 show
pkms task 5 open
pkms task 5 state WAITING
pkms task 5 done
pkms task 5 mod title:"New title" sch:tomorrow
pkms task 5 mod sch:
pkms task 5 mod dl:
pkms task 5 mod dep:2
pkms task 5 mod dep:
pkms task 5 postpone
pkms task 5 postpone --to 2026-06-01
```

Valid states come from `[agenda].open_todo_states` and
`[agenda].closed_todo_states`. `done` uses the first configured closed state.
`--dry-run` is available for state and done. Parent tasks cannot close while an
open child remains.

`mod` requires explicit modifiers and fails on unknown tokens. Empty schedule,
deadline, project, description, tags, priority, or dependency values clear the
corresponding property. A no-op modification prints `Nothing changed` and exits
nonzero.

`postpone` applies only to recurring planned tasks. Without `--to`, it advances
the recurrence while preserving repeater and warning syntax.

Mutations that change canonical ID assignment print a warning on stderr after
normal output. Structured stdout remains parseable.
