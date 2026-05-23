# PKMS Task System

This note records the current task functionality, settled design decisions, and
implementation philosophy for `pkms` task work. It replaces the older
phase-oriented implementation plan: the local task namespace and Todoist-backed
task workflows are already available in the current codebase.

User-facing command details belong in `docs/todo-agenda.md`,
`docs/commands.md`, and `skills/pkms-manager/references/task.md`. Keep this
file focused on design intent and future-change guardrails.

## Philosophy

`pkms` is a non-interactive, terminal-first task tool over an org-roam knowledge
base, with optional Todoist integration for phone capture and cross-device task
inbox use.

The task system should stay:

- Stateless: each command reads current source data, computes output, prints,
  and exits.
- Scriptable: no prompts, TUI, hidden last-result state, or view-local command
  IDs.
- Source-neutral where useful: local PKMS tasks and Todoist tasks share one
  model for list, agenda, reports, and structured output.
- Conservative with local edits: PKMS writes are limited to explicit TODO state
  changes.
- Explicit with remote edits: Todoist mutations require stable
  `todoist:<remote-id>` targets and a build with the `todoist` feature.
- Plain by default: compact text output first, with JSON and NDJSON for agents
  and pipelines.

Taskwarrior remains a UX reference for terminal filtering and task-first
commands, but full Taskwarrior grammar or storage compatibility is not a goal.
Todoist is a source integration, not a sync layer.

## Available Functionality

The legacy local task commands are stable compatibility commands:

```bash
pkms todo
pkms agenda
pkms show <id>
pkms open <id>
```

They share deterministic canonical PKMS task IDs and keep their existing text
columns and JSON shapes.

The newer task namespace is available:

```bash
pkms task list
pkms task agenda
pkms task today
pkms task overdue
pkms task upcoming --days 7
pkms task show p<id>
pkms task open p<id>
pkms task state p<id> WAITING
pkms task done p<id>
pkms task report --today --source all --output-format json
pkms task plan --today --source all --output-format json
pkms task clarify --source todoist --output-format json
```

With `--features todoist`, Todoist-backed reads and writes are also available:

```bash
pkms task list source:todoist
pkms task agenda --today source:todoist
pkms task list source:all
pkms task inbox
pkms task projects source:todoist
pkms task labels source:todoist
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24
pkms task done todoist:<remote-id>
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule todoist:<remote-id> --due none
pkms task update todoist:<remote-id> --title "Call Alice"
pkms task delete todoist:<remote-id> --dry-run
pkms task reopen todoist:<remote-id>
```

Todoist is fetched only when the selected source set includes Todoist. Local
PKMS commands such as `pkms task list source:pkms` must not require network
access or Todoist configuration.

## Source Model

The source-neutral task model lives under `src/tasks/`:

```text
src/tasks/
  mod.rs
  model.rs
  id.rs
  filter.rs
  pkms.rs
  todoist.rs
```

`todoist.rs` is compiled only with the non-default `todoist` Cargo feature.

`TaskItem` is the common shape used by task namespace list, agenda, report,
planning, clarification, Todoist reads, and Todoist mutation output. It includes
stable source identity, display identity, title/body, status/state, priority,
dates, tags, project metadata, PKMS note references, source path/line when
available, URL, and overdue state.

Keep PKMS-specific location fields optional. Todoist tasks should not have to
pretend they came from a local org heading, and PKMS tasks should not require
remote metadata.

## ID Strategy

Supported PKMS target forms:

- `12`: PKMS canonical task shorthand.
- `p12`: explicit PKMS task display ID.
- `pkms:12`: explicit PKMS task ID for scripts.

Supported Todoist target form:

- `todoist:<remote-id>`: stable Todoist task ID.

Unsupported:

- View-local Todoist IDs such as `t4`.
- Bare Todoist remote IDs.
- Hidden "last listed task" references.

Rationale: PKMS canonical IDs already exist and are deterministic while files do
not change. Todoist remote IDs are already stable. View-local remote aliases
would require a cache or hidden command state, which violates the stateless CLI
model.

Text output may use friendly display IDs, but JSON and NDJSON must include
stable source identity fields.

## Filters And Sources

The task namespace accepts a deliberately small filter surface:

```text
source:pkms
source:todoist
source:all
todoist.filter:"today | overdue"
```

The default source is `pkms` for local list, agenda, and shortcut commands.
`task inbox` defaults to Todoist because PKMS has no local inbox convention.

`todoist.filter:` is passed through to Todoist's server-side filter endpoint.
It is valid only when the selected source set includes Todoist, and it takes
precedence over generated Todoist agenda filters.

Todoist agenda flag mapping:

| Flag | Todoist filter |
|------|----------------|
| `--today` | `today` |
| `--overdue` | `overdue` |
| `--week` | `next 7 days` |
| `--upcoming` | `due after: today` |

Do not expand this into a broad boolean expression language without a concrete
use case and tests. Existing top-level `todo` and `agenda` flags remain the
more detailed PKMS-local filtering surface.

## Output Contracts

Task namespace table output uses source-neutral columns:

```text
Id,Source,Date,State,Prio,Tags,Project,Task
```

Compatibility views keep the older PKMS-only columns:

```text
Id,Date,State,Type,Prio,Tags,Note,Heading
```

JSON output should preserve complete source-neutral task fields. NDJSON output
should print one task record per line where the command is stream-like. Agent
workflow commands such as `task report`, `task plan`, and `task clarify` should
prefer stable JSON structures over prose-heavy text.

Do not add color or rich terminal dependencies without a specific design and
accessibility reason.

## Local PKMS Writes

The only supported local PKMS task mutation is changing a heading TODO keyword:

```bash
pkms task state p5 WAITING
pkms task done p5
pkms task done p5 --dry-run
```

Rules:

- Locate the target by canonical PKMS task ID.
- Reject Todoist targets for `task state`.
- Reject headings that no longer have a TODO keyword.
- Valid states come from configured `open_todo_states` and
  `closed_todo_states`.
- State input is case-insensitive.
- The file is written with the configured canonical state spelling.
- `task done` uses the first configured closed state, defaulting to `DONE`.
- Preserve heading title, priority, tags, scheduled/deadline metadata, body, and
  surrounding file content.
- Do not add close timestamps unless the project later adopts a clear org
  convention for them.

PKMS task creation remains intentionally out of scope. A local task creation
command needs a separate destination note/location policy before it can be safe.

## Todoist Integration

Todoist support is behind the non-default `todoist` Cargo feature:

```bash
cargo build --features todoist
```

Configuration is optional and disabled by default:

```toml
[todoist]
enabled = false
token_env = "TODOIST_API_TOKEN"
default_filter = "today | overdue"
```

`[todoist].token` is supported, but environment variables are preferred. Never
print token values. `TODOIST_API_TOKEN` is the default token environment
variable, and `PKMS_TODOIST_API_BASE_URL` exists for tests and mock servers.

Todoist responsibilities currently include:

- Read list, agenda, shortcuts, show, projects, and labels.
- Server-side `todoist.filter:` filtering with pagination.
- Source-neutral mapping into `TaskItem`.
- Quick Add task creation from positional text.
- Structured task creation with title, due date, deadline, project, labels,
  priority, description, and optional PKMS note marker.
- Completion, reopening, postponing, scheduling/unscheduling, update, and
  delete.
- Dry-run support for completion and deletion.

Todoist task IDs used for mutation must be `todoist:<remote-id>`.

`--note <uuid-or-title>` appends a durable `pkms:id:<uuid>` marker to Todoist
descriptions. Listing or showing Todoist tasks detects that marker and fills
PKMS note fields when the note exists locally. Treat this as a privacy boundary:
the PKMS UUID leaves the local database.

## Assistant-Oriented Workflows

Prefer these commands when an agent needs structured task context:

```bash
pkms task report --today --source all --output-format json
pkms task plan --today --source all --output-format json
pkms task clarify --source todoist --output-format json
```

`report` and `plan` group source-neutral tasks into sections such as overdue,
today timed, today untimed, high priority, waiting or blocked, inbox or no date,
and upcoming. `clarify` is read-only and highlights tasks missing planning
information such as date, project, context, or a specific enough title.

These commands are not interactive planners. They provide structured input for
humans or agents to make decisions explicitly.

## Non-Goals

Do not add these without a new explicit design:

- TUI or prompt-driven task processing.
- Generic `task modify` or free-form local org editing.
- PKMS task creation.
- Persistent caches, daemons, watch mode, background sync, or hidden state.
- View-local Todoist command IDs.
- Bidirectional Todoist/PKMS sync.
- Taskwarrior storage integration.
- Full Taskwarrior filter language.
- Local dependency/blocking model.
- Recurrence semantics beyond displaying source data.

## Future-Change Checklist

When changing task behavior:

- Preserve the stateless single-run CLI invariant.
- Keep top-level `todo`, `agenda`, `show`, and `open` compatible unless a
  migration is explicitly planned.
- Update `docs/todo-agenda.md`, `docs/commands.md`, and
  `skills/pkms-manager/references/task.md` for user-visible command behavior.
- Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.
- Add focused integration tests under `tests/integration/`.
- Verify default and relevant feature builds.

Use the full repository verification sequence from `AGENTS.md` before finalizing
code changes:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo build --features=embed
cargo test
cargo test --test integration
cargo run -- --help
```
