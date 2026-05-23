# Plan: Extend `pkms` Into A Non-Interactive Multi-Source Task CLI

## Context

There are 3 task operating programs:

- `pkms`: Dmitry's org-roam PKMS CLI. This is the codebase to extend.
- `taskwarrior`: reference implementation for terminal-first task UX and filter grammar ideas.
- `tod`: reference implementation for Todoist API access and Todoist task data modeling.

The goal is to make `pkms` a pleasant terminal task tool that:

- Works with TODO headings spread across the PKMS.
- Shows agenda/task views without leaving the terminal.
- Feels closer to Taskwarrior than to an interactive prompt tool.
- Can include Todoist tasks, primarily because Todoist is useful for phone capture.

Hard constraints:

- No interactive modes.
- No prompts.
- No TUI.
- No generic `modify` or `edit` command.
- No persistent cache, daemon, watch mode, or background sync.
- No PKMS task creation until the destination note/location policy is designed.
- Todoist support must not force network calls for local PKMS-only commands.

## Current `pkms` Capabilities To Preserve

`pkms` already has a solid local task foundation:

- `todo`, `agenda`, `show`, and `open` share one canonical task ID space.
- Task IDs are deterministic while the underlying files do not change.
- TODO and agenda extraction already includes:
  - note UUID
  - note title
  - file path
  - heading title
  - heading level
  - line number
  - TODO state
  - org priority
  - scheduled/deadline raw values
  - scheduled/deadline normalized dates
  - filetags and heading tags
  - overdue status
- Commands already support `text`, `json`, and `ndjson`.
- Existing pipeline behavior via `--from-stdin` should remain intact.

Do not break current commands:

```bash
pkms todo
pkms agenda
pkms show <id>
pkms open <id>
```

Treat them as stable compatibility commands. They may delegate to shared task
code later, but their text columns and JSON shapes should not change unless
explicitly migrated.

## Resolved Design Decisions

These choices settle the previously undecided or contradictory parts of the plan.

- Add a new `task` namespace, but keep top-level task commands as compatibility
  aliases.
- Introduce the source-neutral task model in the first implementation phase,
  before Todoist. This prevents the `task` namespace from depending directly on
  legacy `todo`/`agenda` row structs and makes later multi-source output a small
  extension instead of a rewrite.
- Phase 1 is read-only and PKMS-only: `task list`, `task agenda`, `task show`,
  and `task open`.
- Phase 2 adds the only local write family: PKMS task state changes. `task
  state` sets any configured TODO state, and `task done` is a convenience
  shorthand for setting the configured closed state.
- Todoist read support comes after local namespace and PKMS writes are stable.
- Todoist creation and Todoist completion are separate later phases.
- No view-local Todoist IDs in commands. Use only stable remote IDs:
  `todoist:<remote-id>`.
- Display IDs may be friendly (`p12` for PKMS, abbreviated Todoist remote IDs in
  text tables), but operation targets must resolve to stable IDs.
- No local Todoist mapping cache. This preserves the project's stateless CLI
  invariant.
- `source:all` is allowed for read commands only. Write commands require a
  single unambiguous target.
- The first filter grammar is small, explicit, and source-neutral. Existing
  top-level `todo`/`agenda` flags stay as they are.
- Todoist authentication uses `TODOIST_API_TOKEN` first. Config may name the env
  var, but secrets should not be stored in config by default.
- Todoist HTTP support should be behind a non-default `todoist` feature so
  default local builds stay dependency-light and network-free.

## Recommended Top-Level Design

Add a new `task` namespace to `pkms`:

```bash
pkms task list
pkms task agenda
pkms task show <id>
pkms task open <id>
pkms task state <id> <state>
pkms task done <id>
pkms task add --source todoist "Buy milk tomorrow"
```

Do not add:

```bash
pkms task modify ...
pkms task edit ...
pkms task add --source pkms ...
pkms task add --interactive ...
```

PKMS task creation remains out of scope.

## Source-Neutral Task Model

Add a dedicated task module:

```text
src/tasks/
  mod.rs
  model.rs
  id.rs
  filter.rs
  pkms.rs
  todoist.rs
```

`todoist.rs` should be compiled only with the `todoist` feature.

Suggested source trait:

```rust
pub trait TaskSource {
    fn source_name(&self) -> &'static str;
    fn list(&self, query: &TaskQuery) -> anyhow::Result<Vec<TaskItem>>;
    fn show(&self, id: &TaskId) -> anyhow::Result<TaskItem>;
    fn set_state(
        &self,
        id: &TaskId,
        state: &str,
        dry_run: bool,
    ) -> anyhow::Result<TaskStateChangeOutput>;
}
```

Recommended common model:

```rust
pub enum TaskSourceKind {
    Pkms,
    Todoist,
}

pub enum TaskId {
    PkmsCanonical(usize),
    TodoistRemote(String),
}

pub struct TaskItem {
    pub id: TaskId,
    pub display_id: String,
    pub source: TaskSourceKind,
    pub source_id: String,
    pub title: String,
    pub body: Option<String>,
    pub status: TaskStatus,
    pub priority: Option<TaskPriority>,
    pub scheduled: Option<TaskDate>,
    pub deadline: Option<TaskDate>,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub note_title: Option<String>,
    pub note_uuid: Option<String>,
    pub path: Option<PathBuf>,
    pub line_number: Option<usize>,
    pub url: Option<String>,
    pub is_overdue: bool,
}
```

Keep PKMS-specific source location fields optional so Todoist tasks can share
the same output table and JSON shape.

## ID Strategy

Supported command target forms:

- `12`: PKMS canonical task shorthand.
- `p12`: explicit PKMS canonical task ID.
- `pkms:12`: explicit PKMS canonical task ID for scripts.
- `todoist:<remote-id>`: stable Todoist task ID.

Unsupported command target forms:

- `t4` or any other view-local Todoist ID.
- Bare Todoist remote IDs.

Rationale:

- PKMS canonical IDs already exist and are stable enough for current local
  commands.
- Todoist IDs are already stable remote identifiers.
- View-local Todoist IDs would require a cache or hidden last-result state,
  which conflicts with the stateless CLI invariant.

Text output can still display short friendly values when useful, but JSON and
NDJSON must include the stable `id`, `source`, and `source_id`.

## Filter Grammar

Add a Taskwarrior-inspired filter parser for the new `task` namespace only. It
supplements existing flags; it does not replace top-level `todo`/`agenda` flags.

Initial supported filters:

```text
source:pkms
source:todoist
source:all
status:open
status:done
state:TODO
state:DONE
tag:agenda
tag:-agenda
priority:A
deadline:today
deadline:tomorrow
deadline:week
deadline.before:2026-06-01
deadline.after:2026-05-01
scheduled:today
scheduled:tomorrow
scheduled:week
scheduled.before:2026-06-01
scheduled.after:2026-05-01
note:"Project Foo"
project:"Inbox"
todoist.filter:"today | overdue"
```

Accepted aliases:

- `src` for `source`
- `prio` for `priority`
- `dead` and `due` for `deadline`
- `sched` and `sch` for `scheduled`
- `before`/`bef` suffixes for `.before`
- `after`/`aft` suffixes for `.after`
- `tod` for `today`
- `tom` for `tomorrow`

Parsing rules:

- Terms are whitespace-separated.
- Quoted strings are preserved.
- Terms combine with implicit AND.
- Repeated `source:` terms are ORed.
- Repeated include `tag:` terms require all listed tags.
- Exclude tags use `tag:-name`.
- Unknown filters fail with a clear error.
- Ambiguous aliases fail with a clear error.
- Boolean expressions are out of scope.
- Full Taskwarrior compatibility is out of scope.

`todoist.filter:` is passed through only for Todoist API reads. It is rejected
when Todoist is not enabled or when the selected source set excludes Todoist.

## Commands: Phase 1, Local PKMS Read Namespace

Goal: introduce the command shape and source-neutral local adapter without
changing storage or adding network.

Add:

```bash
pkms task list [filters...] [--sort <fields>] [--limit <n>] [--columns <cols>] [--output-format <fmt>]
pkms task agenda [filters...] [--today] [--week] [--overdue] [--upcoming] [--output-format <fmt>]
pkms task show <id>
pkms task open <id> [--editor <cmd>] [--line <line>]
```

Behavior:

- Default source is `pkms`.
- `task list` returns source-neutral rows adapted from current `todo` records.
- `task agenda` returns source-neutral rows adapted from current `agenda`
  records.
- `task show` accepts `12`, `p12`, and `pkms:12` and delegates to existing
  canonical ID resolution.
- `task open` accepts `12`, `p12`, and `pkms:12` and delegates to existing
  canonical ID resolution.
- `source:todoist` and `source:all` fail clearly until the `todoist` feature and
  implementation exist.
- Existing `pkms todo`, `pkms agenda`, `pkms show`, and `pkms open` remain
  unchanged.

Do not add writes or Todoist in this phase.

## Commands: Phase 2, PKMS Task State Changes

Add:

```bash
pkms task state <id> <state> [--dry-run] [--output-format <fmt>]
pkms task done <id> [--dry-run] [--output-format <fmt>]
```

Supported targets:

- `12`
- `p12`
- `pkms:12`

Behavior:

- Locate heading by canonical task ID.
- Reject non-PKMS targets.
- Reject headings that do not currently have a TODO keyword.
- `task state` changes the TODO keyword to the requested state.
- Valid target states come from config: `open_todo_states` plus
  `closed_todo_states`.
- State matching is case-insensitive. For example, `done`, `Done`, and `DONE`
  all match configured `DONE`.
- When a requested state matches config case-insensitively, write the canonical
  spelling from config into the org heading.
- Reject unknown states with an error that lists the configured valid states.
- `task done` is shorthand for `task state <id> <closed-state>`.
- `task done` uses the first configured `closed_todo_states` entry, defaulting
  to `DONE`.
- Preserve heading text, tags, priority, scheduled/deadline metadata, body, and
  surrounding file content.
- Do not add a closing timestamp in this phase. There is no current project
  convention to preserve.
- `--dry-run` prints the planned file, line, old state, and new state without
  writing.
- JSON output returns the same fields plus `dry_run`.

No generic modification command. State changes are the complete local edit
surface for this phase.

## Commands: Phase 3, Todoist Read Support

Add the `todoist` Cargo feature and optional Todoist configuration:

```toml
[todoist]
enabled = false
token_env = "TODOIST_API_TOKEN"
default_filter = "today | overdue"
```

Add:

```bash
pkms task list source:todoist
pkms task agenda source:todoist
pkms task list source:all
pkms task agenda source:all
pkms task show todoist:<remote-id>
```

Todoist read behavior:

- Fetch Todoist tasks only when the resolved source set includes Todoist.
- Never fetch Todoist for `source:pkms`.
- Require `--features todoist` at build time.
- Require `todoist.enabled = true` or explicit `source:todoist`/`source:all`.
- Load token from the env var named by `todoist.token_env`; default to
  `TODOIST_API_TOKEN`.
- If no `todoist.filter:` is provided, use `todoist.default_filter` when set.
- Handle Todoist pagination.
- Map Todoist API errors into concise actionable CLI errors.

Useful `tod` code to study:

- `tod/src/tasks.rs` for Todoist task structs.
- `tod/src/todoist.rs` for filter query pagination.
- `tod/src/todoist/request.rs` for authenticated request helpers.

Do not copy `tod`'s prompt/input layer.

Official Todoist API sources verified for the implementation:

- Todoist API v1 uses `GET /api/v1/tasks` and paginated responses with
  `results` plus `next_cursor`.
- Todoist API v1 moved filter queries from `/tasks?filter=` to the dedicated
  `GET /api/v1/tasks/filter` endpoint.
- Todoist API v1 uses stable task IDs in paths such as
  `/api/v1/tasks/{task_id}`.

Source: <https://developer.todoist.com/api/v1/>

## Commands: Phase 4, Todoist Capture

Add only Todoist task creation:

```bash
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --project Inbox "Buy milk tomorrow"
```

Behavior:

- Use Todoist quick-add semantics if practical.
- Require a Todoist token.
- Return the created remote ID in JSON and text output.
- Reject `--source pkms`.

PKMS add remains deferred.

## Commands: Phase 5, Todoist Done

Extend:

```bash
pkms task done todoist:<remote-id> [--dry-run] [--output-format <fmt>]
```

Behavior:

- Todoist: call Todoist complete endpoint.
- `--dry-run` validates the target and prints the planned remote operation
  without calling the complete endpoint.
- Return concise text on success.
- Support JSON output.

PKMS `done` behavior remains as defined in Phase 2.

## Output Requirements

Text output should remain compact and terminal-first.

Default columns for `pkms task list` and `pkms task agenda`:

```text
Id,Source,Date,State,Prio,Tags,Project,Task
```

For PKMS-only compatibility views, keep the existing columns:

```text
Id,Date,State,Type,Prio,Tags,Note,Heading
```

JSON output should include all source-neutral fields, including source-specific
location data where available.

NDJSON output should print one task per line.

Do not introduce color as a dependency unless there is a clear design and
accessibility reason. Plain output first.

## Config

Extend existing `pkms` config conservatively:

```toml
[tasks]
default_sources = ["pkms"]
default_closed_state = "DONE"
default_columns = ["Id", "Source", "Date", "State", "Prio", "Tags", "Project", "Task"]

[todoist]
enabled = false
token_env = "TODOIST_API_TOKEN"
default_filter = "today | overdue"
```

Rules:

- `tasks.default_sources` applies only to the `task` namespace.
- `tasks.default_closed_state` is used only when `closed_todo_states` is empty.
- `closed_todo_states[0]` remains the preferred PKMS closed state.
- Do not store Todoist secrets in config by default.

## Tests

Add tests in `tests/integration`.

Phase 1 tests:

- `task list` matches `todo` item count for fixture DB.
- `task agenda` matches `agenda` item count for fixture DB.
- `task show p1` works.
- `task show 1` works as PKMS shorthand.
- `task show pkms:1` works.
- `task open p1 --editor true` works.
- `task list source:todoist` fails clearly before Todoist support is enabled.
- JSON and NDJSON output are valid.
- Existing `todo`, `agenda`, `show`, and `open` tests still pass.

Phase 2 tests:

- `task done p1 --dry-run` reports planned change and does not edit file.
- `task done p1` edits TODO state to the closed state.
- `task done pkms:1` works.
- `task state p1 WAITING` edits TODO state to `WAITING` when configured.
- `task state p1 waiting` writes the configured canonical spelling `WAITING`.
- `task state p1 Done` writes the configured canonical spelling `DONE`.
- `task state p1 UNKNOWN` fails and lists valid configured states.
- `task done todoist:123` is rejected before Todoist done support exists.
- `task done` and `task state` reject non-task targets.
- JSON output is valid.

Phase 3 Todoist tests:

- Use a mock HTTP server.
- Token can come from env.
- `source:pkms` makes no Todoist request.
- `source:todoist` fetches Todoist tasks.
- `source:all` returns PKMS and Todoist tasks.
- Todoist pagination is handled.
- Todoist API errors produce clear errors.
- `todoist.filter:` is passed through exactly.

Phase 4/5 tests:

- Todoist quick-add sends expected payload.
- Todoist done sends expected complete request.
- Todoist dry-run does not call the complete endpoint.
- PKMS done and Todoist done produce source-specific JSON success output.

## Non-Goals

Do not implement these until explicitly requested:

- TUI.
- Prompt-driven workflows.
- Interactive task processing.
- Generic task editing.
- Recurrence semantics beyond displaying existing source data.
- Dependencies/blocking model.
- Full Taskwarrior filter language.
- Taskwarrior storage integration.
- Bidirectional Todoist/PKMS sync.
- PKMS task creation.
- A daemon, cache, or background sync process.
- View-local Todoist command IDs.

## Branch And Commit Plan

Work on a dedicated branch, for example:

```bash
git switch -c feature/task-cli
```

The implementation is expected to be a long autonomous branch, not a sequence of
handoff points. Keep commits small enough to review, but keep working until the
full task CLI plan is implemented or a real blocker appears.

Commit rules:

- Do not mix unrelated refactors with behavior changes.
- Each commit should leave the project compiling.
- Run focused tests before each commit when practical.
- Run the full verification sequence from `AGENTS.md` before the final commit.
- If a commit changes CLI behavior, include the matching docs and
  `skills/pkms-manager/` updates in the same commit or the immediately following
  docs commit.
- Use clear commit subjects such as `task: add task namespace` or
  `task: add pkms state changes`.

### Commit 1: Add Task Namespace Skeleton

Description: Add `pkms task` with subcommands wired through `src/cli.rs` and
`src/main.rs`, returning clear not-yet-implemented errors where behavior is not
ready.

Acceptance criteria:

- `pkms task --help` shows the planned subcommands.
- Existing top-level commands are unchanged.
- The command structure leaves room for source-neutral task handling.

Verification:

- Add or update command-dispatch/help coverage if needed.
- Run focused CLI tests.

### Commit 2: Add Source-Neutral Task Model And PKMS Adapter

Description: Add `src/tasks/` with source-neutral IDs, filters, model structs,
and a PKMS adapter over existing `TaskRecord` collection logic.

Acceptance criteria:

- PKMS todo and agenda records can be represented as `TaskItem`.
- IDs support `12`, `p12`, and `pkms:12`.
- Todoist IDs are parsed only as stable `todoist:<remote-id>` values.
- Existing top-level commands are unchanged.

Verification:

- Add unit tests for ID parsing and model conversion.
- Run focused task/model tests.

### Commit 3: Implement PKMS `task list` And `task agenda`

Description: Implement read-only PKMS task views in the new namespace using the
source-neutral model.

Acceptance criteria:

- `task list` and `task agenda` work for PKMS data.
- Default source is `pkms`.
- `source:todoist` and `source:all` fail clearly before Todoist support exists.
- Text, JSON, and NDJSON output are valid.
- Existing `todo` and `agenda` behavior is unchanged.

Verification:

- Add integration tests for counts, output formats, and unsupported sources.
- Run focused `task list`/`task agenda` tests.

### Commit 4: Implement PKMS `task show` And `task open`

Description: Add `task show` and `task open` as namespace equivalents of the
existing canonical task workflows.

Acceptance criteria:

- `task show` and `task open` accept `12`, `p12`, and `pkms:12`.
- Non-PKMS Todoist targets fail clearly until Todoist read support exists.
- Existing `show` and `open` behavior is unchanged.

Verification:

- Add integration tests for all supported PKMS target forms.
- Run focused show/open tests.

### Commit 5: Implement PKMS Task State Changes

Description: Add `pkms task state` and `pkms task done` with `--dry-run`.

Acceptance criteria:

- The command changes only the TODO keyword.
- Dry-run reports the planned change without editing.
- Valid states are read from config.
- State input is case-insensitive and writes canonical config spelling.
- Closed-state selection for `done` follows config.
- Non-PKMS targets, non-task targets, and unknown states fail clearly.

Verification:

- Add integration tests using fixture files.
- Run focused state-change tests.

### Commit 6: Document Local Task Namespace

Description: Document the PKMS-only task namespace before adding network-backed
Todoist behavior.

Acceptance criteria:

- `docs/commands.md` includes `task list`, `task agenda`, `task show`,
  `task open`, `task state`, and `task done`.
- `docs/todo-agenda.md` explains `p<N>`, `pkms:<N>`, state changes, and
  configured state normalization.
- `docs/workflows.md` includes the terminal task workflow.
- `skills/pkms-manager/` schemas and references cover the new local commands.

Verification:

- Run the full default verification sequence from `AGENTS.md`.

### Commit 7: Add Todoist Feature Flag And HTTP Client Foundation

Description: Add the non-default `todoist` Cargo feature, config fields, token
resolution, and a small Todoist HTTP client.

Acceptance criteria:

- Default builds do not include Todoist HTTP dependencies unless avoidable.
- Todoist token is read from the configured env var, defaulting to
  `TODOIST_API_TOKEN`.
- Missing-token and disabled-config errors are clear.
- No Todoist network request happens for `source:pkms`.

Verification:

- Add unit tests for config/token resolution.
- Run default and `--features todoist` builds.

### Commit 8: Implement Todoist Read Support

Description: Add Todoist list, agenda, and show behavior through the
source-neutral task model.

Acceptance criteria:

- `source:todoist` and `source:all` work with env-token auth.
- Todoist pagination is handled.
- `todoist.filter:` is passed through exactly.
- JSON/NDJSON include stable Todoist IDs.
- Todoist API errors produce clear CLI errors.

Verification:

- Add mock HTTP integration tests.
- Run default verification and Todoist feature verification.

### Commit 9: Document Todoist Read Support

Description: Update user docs and agent skill references for Todoist read
behavior.

Acceptance criteria:

- Docs explain enabling the `todoist` feature.
- Docs explain `TODOIST_API_TOKEN`, config, `source:todoist`, `source:all`, and
  `todoist.filter:`.
- `skills/pkms-manager/` references and schemas include stable Todoist IDs.

Verification:

- Run focused docs/schema checks where available.

### Commit 10: Implement Todoist Capture

Description: Add `pkms task add --source todoist` using quick-add semantics.

Acceptance criteria:

- Todoist quick-add works from the CLI.
- JSON output returns the created remote ID.
- `--source pkms` is rejected.

Verification:

- Add mock HTTP tests for payload and output.
- Run default and Todoist feature verification.

### Commit 11: Implement Todoist Done

Description: Extend `pkms task done` to complete `todoist:<remote-id>` targets.

Acceptance criteria:

- Todoist completion calls the expected endpoint.
- Dry-run does not mutate remote state.
- PKMS completion behavior is unchanged.

Verification:

- Add mock HTTP tests for complete and dry-run behavior.
- Run default and Todoist feature verification.

### Commit 12: Final Documentation And Verification

Description: Bring docs, schemas, command references, and examples in line with
the completed branch.

Acceptance criteria:

- `docs/commands.md`, `docs/todo-agenda.md`, `docs/workflows.md`, and
  `skills/pkms-manager/` match the final CLI behavior.
- `README.md` has only a concise mention if needed.
- The branch has no unrelated formatting or metadata churn.

Verification:

- Run the full verification sequence from `AGENTS.md`:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo build --features=embed
cargo test
cargo test --test integration
cargo run -- --help
```

Implementation note: the completed branch uses Todoist API v1 endpoints from
the official Todoist developer docs, including `GET /api/v1/tasks`,
`GET /api/v1/tasks/filter`, `POST /api/v1/tasks/quick`, and
`POST /api/v1/tasks/{task_id}/close`.

## Documentation Updates

When implementing this plan, update:

- `docs/commands.md` for the new `task` namespace.
- `docs/todo-agenda.md` for ID behavior, `task state`, and `task done`.
- `docs/workflows.md` for terminal task workflows.
- `skills/pkms-manager/` schemas and references when CLI behavior changes.

Do not update `README.md` beyond a short mention unless the final command set is
ready for broad user-facing documentation.
