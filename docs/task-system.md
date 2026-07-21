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
pkms task p<ID> show
pkms task p<ID> open
pkms task p<ID> state WAITING
pkms task p<ID> done
pkms task p<ID> mod sch:2026-05-24
pkms task p<ID> mod dl:2026-05-30
pkms task p<ID> mod dep:<parent-id>
pkms task p<ID> postpone
pkms task p<ID> postpone --to tomorrow
```

`task calendar` is a compact text-only view of open PKMS task dates. It shows
the current month by default, or consecutive months from the current month with
positive `-m N` / `--months N`. A negative value includes that many previous
months plus the current month, so `-m -1` shows the previous and current month.
Multiple months are printed horizontally and wrap onto additional rows when the
terminal is too narrow. On ANSI-capable terminals, dates with scheduled tasks
are underlined and dates with deadlines are red. The current day is blue unless
it has a deadline. A compact styled legend precedes the calendar.

With a build that includes the `todoist` feature, task views can include Todoist
tasks when filters select `source:todoist` or `source:all`. Todoist creation uses
`source:todoist`, and supported Todoist ID-first actions use stable
`todoist:<remote-id>` targets:

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
UX reference for terminal filtering, but Taskwarrior grammar and storage are
outside the current `pkms task` surface.

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
  todoist.rs
  todoist_mutation.rs
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
  mutations.rs      # source-neutral mutation dispatch
  mutations/        # PKMS and Todoist mutation adapters
```

Keep provider and mutation logic in `pkms-task` unless it is only command-line
or presentation glue. `pkms-task` owns validation, source selection,
clock-sensitive date windows, provider collection, sorting, limiting, canonical
task IDs, Todoist API execution, and typed local task mutation requests. It
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
pkms task <ID> postpone [--to <DATE>]
pkms task <ID> mod <MODIFIER>...
```

`open` is PKMS-only because provider-backed tasks do not have a local source
heading to open.

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
global ID positions. Single-source text views show bare source IDs: `<ID>` for
PKMS and `<remote-id>` for Todoist. In `source:all` text views, IDs are
disambiguated as `p<ID>` for PKMS and `todoist:<remote-id>` for Todoist. Scripts
should rely on source identity fields in JSON/NDJSON.

`task <ID> show` includes all parent headings and the child task chain for nested
PKMS tasks. Parent TODO headings and every child entry carry the same canonical
task ID used by `task list`, `task agenda`, `task p<ID> show`, and
`task p<ID> open`; ordinary parent headings have a null `id` and `todo_state` in
structured output.

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

Todoist-backed `task agenda` uses the Todoist `!no date` filter by default to
fetch scheduled tasks, then applies local criteria such as `date:today` or
`date:upcoming` to the fetched items. `todoist.filter:<query>` overrides the
Todoist fetch query, but local task criteria still apply. `task inbox
source:todoist` uses Todoist's `#Inbox` filter.

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
- Task-provider reads and mutation execution inside `pkms-task`.
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

The task namespace does not include:

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
cargo test --workspace --all-features
cargo build --workspace --all-features
```
