# Task System Design

This note records the current task design, implementation guardrails, and
maintenance checklist for the `pkms task` namespace. It replaces older planning
notes and documentation for removed top-level task command drafts; task
workflows now live under `pkms task`.

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
pkms task agenda upcoming --days 7
pkms task inbox
pkms task list projects
pkms task list tags
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add dep:2 title:"Follow up on parent task"
pkms task p<ID> show
pkms task p<ID> open
pkms task p<ID> state WAITING
pkms task p<ID> done
pkms task p<ID> mod sch:2026-05-24
pkms task p<ID> mod dl:2026-05-30
pkms task p<ID> mod dep:<parent-id>
pkms task p<ID> postpone --to tomorrow
```

With a build that includes the `todoist` feature, the same views and ID-first
actions can include Todoist tasks when filters select `source:todoist` or
`source:all`:

```bash
pkms task list source:todoist
pkms task agenda today source:all
pkms task inbox source:todoist
pkms task list projects source:all
pkms task list tags source:all
pkms task add source:todoist title:"Call Alice" due:2026-05-24 prio:B
pkms task todoist:<remote-id> show
pkms task todoist:<remote-id> done
pkms task todoist:<remote-id> mod sch:
pkms task todoist:<remote-id> mod dl:
```

Removed or obsolete surfaces:

- No top-level `todo`, `agenda`, `show`, or `open` commands.
- No `task report`, `task plan`, or `task clarify` commands.
- No view-local Todoist IDs as mutation targets.
- No flag-style task creation such as `task add --source todoist --title ...`.
  Task creation uses positional text plus `key:value` add modifiers.

## Philosophy

`pkms` remains a stateless, terminal-first CLI over an org-roam database, with
optional provider-backed task access.

The task system should stay:

- Stateless: each command reads current source data, computes output, prints,
  and exits.
- Scriptable: no prompts, TUI, hidden last-result state, or view-local command
  IDs.
- Source-neutral where useful: PKMS and Todoist tasks share one `TaskItem`
  model for list, agenda, inbox, metadata, show, and supported mutations.
- Conservative with local edits: PKMS writes are explicit heading edits for
  state, schedule, deadline, recurring postponement, and inbox task creation.
- Explicit with remote edits: Todoist mutations require stable
  `todoist:<remote-id>` targets and a `todoist` feature build.
- Plain by default: compact text output first, with JSON and NDJSON for agents
  and pipelines.

Todoist is a source integration, not a sync layer. Taskwarrior remains a useful
UX reference for terminal filtering, but Taskwarrior grammar or storage
compatibility is not a goal.

## Source Model

The source-neutral task model lives under `src/tasks/`:

```text
src/tasks/
  add.rs
  clock.rs
  filter.rs
  id.rs
  mod.rs
  model.rs
  pkms_edit.rs
  pkms_mutation.rs
  pkms.rs
  provider.rs
  scope.rs
  todoist.rs
  todoist_provider.rs
```

`TaskItem` is the common shape used by task views and mutations. It carries
source identity, display identity, title/body, status/state, priority, dates,
tags, project metadata, PKMS note references, source path/line when available,
URL, and PKMS-specific metadata such as daily-file and agenda-tag fields.

Keep source-specific fields optional. Todoist tasks should not need local
heading metadata, and PKMS tasks should not need remote metadata.

Todoist support is compiled behind the non-default `todoist` feature. Local
PKMS commands must not require Todoist configuration or network access unless
the selected source set includes Todoist.

## Implementation Boundaries

Task command orchestration lives under `src/commands/task/`:

```text
src/commands/task/
  mod.rs          # command dispatch, list/agenda request planning and execution
  id_command.rs   # ID-first parsing and dispatch adapter
  providers.rs    # source selection and provider orchestration
  render.rs       # text, JSON, NDJSON, table, and mutation output helpers
  agenda.rs       # PKMS agenda grouping path
  todo.rs         # PKMS TODO listing path
```

Keep provider and mutation logic in `src/tasks/` unless it is only command-line
or presentation glue. List and agenda commands use internal request-planning and
execution helpers before calling centralized rendering. This keeps validation,
source selection, clock-sensitive date windows, provider collection, sorting,
limiting, and output presentation separate enough to test without creating a new
task framework.

Clock-sensitive task behavior should use `TaskClock` from `src/tasks/clock.rs`
captured at the command boundary. Avoid direct scattered calls to local time in
task filtering, agenda grouping, task-add date parsing, and mutation reloads.

## IDs And Actions

Supported PKMS task target forms:

- `12`: PKMS canonical task shorthand.
- `p12`: explicit PKMS display ID.
- `pkms:12`: explicit PKMS source ID for scripts.

Supported provider target form:

- `todoist:<remote-id>`: stable Todoist task ID.

ID-first actions are the preferred command style:

```bash
pkms task <ID> show
pkms task <ID> open [--editor <COMMAND>] [--line <LINE>]
pkms task <ID> state <STATE> [--dry-run]
pkms task <ID> done [--dry-run]
pkms task <ID> postpone --to <DATE>
pkms task <ID> mod <MODIFIER>...
```

Hidden subcommands such as `pkms task show <ID>` may exist internally for clap
dispatch, but docs and examples should prefer ID-first usage.

PKMS canonical IDs are deterministic while files do not change. Filtered views
may show non-contiguous IDs because excluded tasks still occupy global ID
positions. In single-source text views, IDs may be shown as bare source IDs; in
`source:all` views, PKMS display IDs are prefixed with `p` and Todoist display
IDs with `t`. Scripts should rely on source identity fields in JSON/NDJSON.

## Filters And Views

Task item commands accept positional filters after the subcommand. Source
filters select providers:

```text
source:pkms
source:todoist
source:all
src:pkms
src:todoist
src:all
todoist.filter:<query>
```

The default source is `pkms`. `todoist.filter:` is valid only with Todoist in
the selected source set and takes precedence over generated Todoist agenda
filters.

Supported task criteria include:

- `state:TODO`, `state:opened`, or `state:closed`, including
  comma-separated values and `!` exclusions.
- `tags:work,!blocked` or `tag:work`.
- `type:SCHED,DEADL` or `kind:SCHED`.
- `prio:A`, `priority:A,B,C`, and `prio:none`.
- `date:today`, `date:week`, `date:overdue`, `date:upcoming`,
  `date:YYYY-MM-DD`, and comma-separated date filters.
- `after:YYYY-MM-DD`, `before:YYYY-MM-DD`, with optional `HH:MM`.
- `scope:<note-title-uuid-or-path>` for PKMS task scope.
- `project:<name-or-id>`, including `!` exclusions.

Agenda shortcut mapping for Todoist:

| View | Todoist filter |
|------|----------------|
| `task agenda` | `!no date` |
| `task agenda today` | `today` |
| `task agenda week` | `next 7 days` |
| `task agenda overdue` | `overdue` |
| `task agenda upcoming --days N` | `due after: today & next N days` |
| `task inbox source:todoist` | `#Inbox` |

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

Default columns can be configured globally or per source and view:

```toml
[columns.pkms]
tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]

[columns.todoist]
tasks = ["Id", "State", "Prio", "Tags", "Project", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Heading"]
```

For `source:all`, source-specific configured defaults must resolve to the same
column set; otherwise users should pass `--columns` explicitly.

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

Local mutation rules:

- `task <ID> state` changes only the TODO keyword.
- Valid states come from configured `open_todo_states` and
  `closed_todo_states`.
- State input is case-insensitive; files are written with configured canonical
  spelling.
- `task <ID> done` uses the first configured closed state, defaulting to
  `DONE`.
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
- `task <ID> postpone --to DATE` is only for recurring planned tasks and must
  preserve repeater/warning syntax.
- Preserve unmodified heading title, priority, tags, body, surrounding file
  content, and unrelated planning metadata.
- Do not add close timestamps unless the project adopts a clear org convention.

## Todoist Integration

Todoist support requires a `todoist` feature build and configuration:

```toml
[todoist]
enabled = false
token_env = "TODOIST_API_TOKEN"
default_filter = "today | overdue"
```

`[todoist].token` is supported, but environment variables are preferred. Never
print token values. `TODOIST_API_TOKEN` is the default token environment
variable, and `PKMS_TODOIST_API_BASE_URL` exists for tests and mock servers.
HTTPS uses the platform certificate verifier so system trust-store corporate
proxy roots are honored.

For debugging Todoist API behavior, set `PKMS_LOG_HTTP=1`. It writes HTTP
metadata and pagination counts to stderr without logging tokens, request
bodies, task content, or descriptions.

Todoist responsibilities currently include:

- Read list, agenda, inbox, projects, labels, and show.
- Server-side `todoist.filter:` filtering with pagination.
- Source-neutral mapping into `TaskItem`.
- Quick Add creation from positional text.
- Structured creation using add modifiers for title, due/schedule, deadline,
  project, labels, priority, and description.
- Completion, reopening via `state open`, scheduling/unscheduling,
  deadline/clear-deadline, recurring postponement, and dry-run completion.

Todoist task IDs used for mutation must be `todoist:<remote-id>`.

Listing or showing Todoist tasks detects `pkms:id:<uuid>` PKMS note markers in
Todoist descriptions and fills `note_uuid` and `note_title` when the note exists
locally. Treat that marker as a privacy boundary: the PKMS UUID leaves the local
database.

## Non-Goals

Do not add these without a new explicit design:

- TUI or prompt-driven task processing.
- Persistent caches, daemons, watch mode, background sync, or hidden state.
- View-local Todoist command IDs.
- Bidirectional Todoist/PKMS sync.
- Taskwarrior storage integration.
- Full Taskwarrior filter language.
- Generic local org editing beyond the explicit task mutations above.
- Local dependency/blocking model.
- Recurrence semantics beyond displaying source data and postponing recurring
  tasks with source-native recurrence.

## Future-Change Checklist

When changing task behavior:

- Preserve the stateless single-run CLI invariant.
- Keep command examples under the `pkms task` namespace.
- Update `docs/todo-agenda.md`, `docs/commands.md`, and
  `skills/pkms-manager/references/task.md` for user-visible behavior.
- Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.
- Add focused integration tests under `tests/integration/task.rs`.
- Verify default and relevant feature builds.

Use the development workflow in [Development](development.md) before finalizing
code or documentation changes. For normal local work, run the fast pre-commit
gate plus relevant feature-specific checks. CI runs the full gate.

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo test
```
