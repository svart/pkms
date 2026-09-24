# Task System Design

This note records the current task design, implementation rules, and
maintenance checklist for the `pkms task` namespace.

User-facing command details belong in `docs/todo-agenda.md`,
`docs/commands.md`, and `skills/pkms-manager/references/task.md`. Keep this
file focused on durable design intent and constraints for future changes.

## Current State

The task surface is:

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming
pkms task calendar
pkms task calendar --months 3
pkms task calendar -m -1
pkms task inbox
pkms task list projects
pkms task list tags
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add dep:2 title:"Follow up on parent task"
pkms task <ID> show
pkms task <ID> open
pkms task <ID> state WAITING
pkms task <ID> done
pkms task <ID> mod sch:2026-05-24
pkms task <ID> mod dl:2026-05-30
pkms task <ID> mod dep:<parent-id>
pkms task <ID> postpone
pkms task <ID> postpone --to tomorrow
```

`task calendar` is a compact text-only view of open PKMS task dates. It shows
the current month by default, or consecutive months from the current month with
positive `-m N` / `--months N`. A negative value includes that many previous
months plus the current month, so `-m -1` shows the previous and current month.
Multiple months are printed horizontally and wrap onto additional rows when the
terminal is too narrow. On ANSI-capable terminals, dates with scheduled tasks
are underlined and dates with deadlines are red. The current day is blue unless
it has a deadline. A compact styled legend precedes the calendar.

## Philosophy

`pkms` remains a stateless, terminal-first CLI over an org-roam database.

The task system should stay:

- Stateless: each command reads current source data, computes output, prints,
  and exits.
- Scriptable: no prompts, TUI, hidden last-result state, or view-local command
  IDs.
- Stable structured output: local tasks use one `TaskItem` model for list,
  agenda, inbox, metadata, show, and mutations.
- Conservative with local edits: PKMS writes are explicit heading edits for
  state, schedule, deadline, recurring postponement, and inbox task creation.
- Plain by default: compact text output first, with JSON and NDJSON for agents
  and pipelines.

Taskwarrior remains a useful UX reference for terminal filtering, but
Taskwarrior grammar and storage are outside the current `pkms task` surface.

## Source Model

The source-neutral task model lives under `crates/pkms-task/src/`:

```text
  common.rs
  clock.rs
  config.rs
  execution.rs
  filter.rs
  id.rs
  model.rs
  modifiers.rs
  mutation.rs
  pkms.rs
  projection.rs
  provider.rs
  providers.rs
  scope.rs
  show.rs
  task_index.rs
```

`TaskItem` is the common shape used by task views and mutations. It carries
source identity, display identity, title/body, status/state, priority, dates,
tags, project metadata, PKMS note references, source path/line when available,
URL, and PKMS-specific metadata such as daily-file and agenda-tag fields.
For PKMS tasks, tags are the de-duplicated combination of note `#+filetags`,
all parent-heading tags, and the task heading's own tags, in that order.

Keep optional fields stable so existing JSON consumers are not forced to
special-case missing local metadata.

## Implementation Boundaries

Task command orchestration lives under `crates/pkms/src/commands/task/`:

```text
crates/pkms/src/commands/task/
  mod.rs            # namespace dispatch and shared task command wiring
  id_command.rs     # ID-first parsing and action dispatch
  plan.rs           # list, agenda, inbox, source, and filter planning
  providers.rs      # CLI config mapping and provider adapters
  render.rs         # text, JSON, NDJSON, table, and mutation output helpers
  show.rs           # cross-domain show dispatch adapter
  open.rs           # local editor-opening adapter
  mutations.rs      # local mutation dispatch
  mutations/        # PKMS mutation adapters
```

Keep provider and mutation logic in `pkms-task` unless it is only command-line
or presentation glue. `pkms-task` owns validation, clock-sensitive date
windows, local provider collection, sorting, limiting, canonical task IDs, and
typed local task mutation requests. It
builds typed `pkms-org` edit specs for local org writes; it should not construct
raw org task text or write org files directly.

`pkms-org` owns org task syntax, typed task insertion, daily note creation, and
raw org file writes. The umbrella `pkms` task modules parse CLI arguments, map
configuration into task-provider config, dispatch cross-domain show/open
commands, and render text, JSON, and NDJSON output.

Clock-sensitive task behavior should use `TaskClock` from
`crates/pkms-task/src/clock.rs` captured at the command boundary. Avoid direct
scattered calls to local time in task filtering, agenda grouping, task-add date
parsing, and mutation reloads.

## IDs And Actions

PKMS task targets are positive integer canonical IDs such as `12`.

ID-first actions are the preferred command style:

```bash
pkms task <ID> show
pkms task <ID> open [--editor <COMMAND>] [--line <LINE>]
pkms task <ID> state <STATE> [--dry-run]
pkms task <ID> done [--dry-run]
pkms task <ID> postpone [--to <DATE>]
pkms task <ID> mod <MODIFIER>...
```

Hidden subcommands such as `pkms task show <ID>` may exist internally for clap
dispatch, but docs and examples should prefer ID-first usage.

PKMS canonical IDs are deterministic while task status group, file identity,
and heading order do not change. The global ordering is open tasks before
closed tasks, then
timestamped files (`YYYYMMDDHHMMSS-rest.org`) newest to oldest, with daily
files (`YYYY-MM-DD.org`) treated as `YYYY-MM-DD 00:00:00`. Files with equal
timestamps sort by the rest of the filename alphanumerically. Files without a
recognized timestamp sort last by path, and tasks within a file sort by heading
line number. Mutable task properties such as priority, `DEADLINE`,
`SCHEDULED`, tags, and project do not affect canonical IDs. Clock-relative
concepts such as today, overdue, and upcoming do not affect canonical IDs.
Filtered views may show non-contiguous IDs because excluded tasks still occupy
global ID positions. Text views show bare canonical PKMS IDs. Scripts should
rely on source identity fields in JSON/NDJSON.

`task <ID> show` includes all parent headings and the child task chain for nested
PKMS tasks. Parent TODO headings and every child entry carry the same canonical
task ID used by `task list`, `task agenda`, `task <ID> show`, and
`task <ID> open`; ordinary parent headings have a null `id` and `todo_state` in
structured output.

## Filters And Views

Task item commands accept positional filters after the subcommand. Source
filters may explicitly select the local source:

```text
source:pkms
src:pkms
```

The default and only source is `pkms`.

Supported task criteria include:

- `state:TODO`, `state:opened`, or `state:closed`, including
  comma-separated values and `!` exclusions.
- `tags:work,!blocked` or `tag:work`.
- `type:SCHED,DEADL` or `kind:SCHED`.
- `prio:A`, `priority:A,B,C`, and `prio:none`.
- `date:today`, `date:week`, `date:overdue`, `date:upcoming`,
  `date:YYYY-MM-DD`, modifier-style date words such as `date:tom` and
  `date:fri`, and comma-separated date filters.
- `after:YYYY-MM-DD`, `before:YYYY-MM-DD`, optional `HH:MM`, and
  modifier-style date words such as `after:tom` and `before:fri`.
- `scope:<note-title-uuid-or-path>` for PKMS task scope.
- `project:<name-or-id>`, including `!` exclusions.

Agenda shortcut subcommands are aliases for date filters:

| Shortcut | Equivalent filter |
|----------|-------------------|
| `task agenda today` | `task agenda date:today` |
| `task agenda week` | `task agenda date:week` |
| `task agenda overdue` | `task agenda date:overdue` |
| `task agenda upcoming` | `task agenda date:upcoming` |

`task agenda upcoming --days N` is equivalent to
`task agenda --days N date:upcoming`.

Do not expand this into a broad boolean expression language without a concrete
use case and tests.

## Columns And Output

Available table columns:

```text
Id,Date,State,Type,Prio,Tags,Project,Note,Heading
```

`--columns` can replace the active set or adjust configured/default columns:

```bash
pkms task list --columns Id,Heading
pkms task list --columns +Project
pkms task agenda --columns -Project
```

The `Date` column displays task dates as `YYYY-MM-DD Day`. Daily-note tasks
without `SCHEDULED` or `DEADLINE` markers use the daily note date.

Default columns can be configured globally or for PKMS task views:

```toml
[columns.pkms]
tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]
```

JSON output should preserve complete source-neutral task fields. NDJSON output
should print one task record per line for stream-like views. Changed-task JSON
uses wrappers such as `changed: true`, `action`, and `item`; creation JSON uses
`created: true` and `item`.

Do not add color or rich terminal dependencies without a specific design and
accessibility reason.

## PKMS Writes

PKMS task creation appends an org TODO heading. By default it writes to the
configured inbox note:

```toml
[tasks]
inbox = "Inbox"
```

The inbox can be a note title, UUID, absolute path, path relative to `db_root`,
or `daily`. `daily` uses today's daily note and places tasks under a top-level
`* Inbox` heading, creating the heading when needed. New daily notes are created
under `daily_notes_dir`, or `new_notes_dir` when `daily_notes_dir` is unset.
`note:<target>` on `task add` is PKMS-only and overrides the configured inbox
destination.
`dep:<task-id>`/`depend:<task-id>` on `task add` is PKMS-only and appends the
new task as the final child heading in the referenced task's subtree.

PKMS add modifiers:

| Modifier | Aliases | Meaning |
|----------|---------|---------|
| `source:pkms` | `src:` | Select local PKMS creation. |
| `title:<text>` | | Structured task title. |
| `state:<state>` | | Set the org TODO keyword from configured agenda states. |
| `tag:<value>` | `tags:`, `label:`, `labels:` | Add tags; repeat or comma-separate. |
| `schedule:<date>` | `sch:`, `sched:`, `due:` | Add `SCHEDULED`. |
| `deadline:<date>` | `dead:`, `dl:` | Add `DEADLINE`. |
| `project:<value>` | `proj:` | Set `PROJECT` when it differs from the destination note project. |
| `prio:<A-B-C>` | `priority:`, `pri:` | Add source-neutral priority. |
| `desc:<text>` | `description:`, `body:` | Add body text. |
| `note:<target>` | | Choose destination note. |
| `dep:<task-id>` | `depend:` | Add as a child of an existing PKMS task. |

Dates accept unambiguous case-insensitive prefixes of `today`, `tomorrow`, or
weekday names, plus `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`. Weekday names resolve
to the next upcoming matching weekday, so `sch:fri` on a Friday means next
Friday. Ambiguous prefixes fail with an error.
Modifier keys accept documented aliases and unambiguous prefixes; for example
`proj:` and `pro:` resolve to `project:`, while `pr:` is ambiguous between
`project:` and `priority:`.

PKMS project planning belongs in `pkms-task`. It compares an explicitly
requested project with the parsed note-level `PROJECT` value
case-insensitively. Matching values inherit from the note without writing a
heading property; differing values are passed to `pkms-org` as a typed task
insertion or heading mutation property. Changing an existing task to its note's
project removes the heading-level override.

Local mutation rules:

- `task <ID> state` changes only the TODO keyword.
- Valid states come from configured `open_todo_states` and
  `closed_todo_states`.
- State input is case-insensitive; files are written with configured canonical
  spelling.
- `task <ID> done` uses the first configured closed state, defaulting to
  `DONE`.
- Successful PKMS state changes report the resulting canonical task ID because
  moving between open and closed state groups can renumber tasks. Structured
  output keeps the requested ID in `id` and reports the result as `new_id`.
- `task <ID> mod` changes add-style task properties such as `title:`,
  `state:`, `tag:`, `sch:`, `dl:`, `project:`, `prio:`, `desc:`, and `dep:`.
- Unlike `task add`, `task <ID> mod` does not accept positional title text;
  use `title:<text>` to rename a task.
- `task <ID> mod` fails on any unrecognized modifier.
- `task <ID> mod sch:DATE` sets `SCHEDULED`; `task <ID> mod sch:` clears it.
- `task <ID> mod dl:DATE` sets `DEADLINE`; `task <ID> mod dl:` clears it.
- `task <ID> mod dep:<task-id>` moves the whole PKMS task subtree to the end of
  the referenced task's subtree.
- `task <ID> mod dep:` removes the current PKMS task dependency by moving the
  whole subtree out to the end of the parent task's subtree.
- If `task <ID> mod` would not change anything, it prints `Nothing changed`
  and exits nonzero.
- `task <ID> postpone [--to DATE]` is only for recurring planned tasks. Without
  `--to`, it advances to the next occurrence; an explicit date overrides that
  default. PKMS mutations preserve repeater/warning syntax.
- Preserve unmodified heading title, priority, tags, body, surrounding file
  content, and unrelated planning metadata.
- Do not add close timestamps unless the project adopts a clear org convention.

## Non-Goals

The task namespace does not include:

- TUI or prompt-driven task processing.
- Persistent caches, daemons, watch mode, background sync, or hidden state.
- Remote provider command IDs or sync.
- Taskwarrior storage integration.
- Full Taskwarrior filter language.
- Generic local org editing beyond the explicit task mutations above.
- Local dependency/blocking model.
- Recurrence semantics beyond displaying source data and postponing recurring
  tasks with source-native recurrence.

## Maintenance Checklist

When changing task behavior:

- Preserve the stateless single-run CLI invariant.
- Keep command examples under the `pkms task` namespace.
- Update `docs/todo-agenda.md`, `docs/commands.md`, and
  `skills/pkms-manager/references/task.md` for user-visible behavior.
- Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.
- Add focused integration tests under `tests/integration/task.rs`.
- Verify the all-features pre-commit gate.

Use the development workflow in [Development](development.md) before finalizing
code or documentation changes. For normal local work, run the fast all-features
pre-commit gate. Run the full matrix only when explicitly requested.

```bash
cargo fmt --all -- --check
scripts/check-crate-boundaries.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```
