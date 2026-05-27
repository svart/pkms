# Codebase Improvement Plan

## Purpose

This document records a pragmatic plan for simplifying future development in
`pkms` without changing the project's core architecture.

The current architecture is fundamentally sound:

- `pkms` remains a stateless single-run CLI.
- The binary is thin and delegates to library code.
- CLI parsing, configuration resolution, corpus loading, graph construction, and
  command execution are separated.
- Integration tests exercise the real binary and preserve CLI contracts.

The main improvement opportunity is not a rewrite. It is reducing large
responsibility clusters so new features can be added with less context and less
risk.

## Current Architecture Summary

The main execution path is:

1. `src/main.rs` initializes logging, parses `Cli`, and calls `runner::run`.
2. `src/runner.rs` builds `App`, resolves config, and dispatches commands.
3. Command modules under `src/commands/` load a `Graph` or `Workspace` when
   needed.
4. `Corpus::load()` discovers and parses org files.
5. `Graph::from_corpus()` builds the in-memory graph.
6. Command output is printed through `OutputContext` or direct text rendering.

This shape should be preserved. The stateless model keeps behavior predictable,
keeps tests close to real usage, and avoids hidden derived state.

## Keep These Invariants

- Do not introduce persistent caches, background daemons, or watch mode.
- Keep `Graph::load()` and `Workspace::load()` as fresh per-invocation loading
  paths.
- Keep command behavior available through binary-level integration tests.
- Keep command output serializable for JSON and NDJSON.
- Prefer small module extraction over framework-style rewrites.
- Add abstractions only where they remove real repeated complexity.

## Main Friction Points

### Task Command Module

`src/commands/task/mod.rs` is the largest development bottleneck. It mixes:

- task command dispatch
- ID-first compatibility parsing
- provider orchestration
- PKMS and Todoist source selection
- list, agenda, inbox, metadata, and mutation flows
- filtering and sorting
- table rendering and output formatting

Future task features currently require understanding too much unrelated code.

### Serve Command Module

`src/commands/serve.rs` is another good split candidate. It mixes:

- TCP listener lifecycle
- HTTP routing and response writing
- asset serving
- full-page and preview rendering
- org-to-HTML rendering
- inline formatting
- syntax highlighting
- CSS and JavaScript embedding

This module has clearer split boundaries than most commands, and splitting it
would make the rendering code much easier to unit test.

### Direct Output and Runtime Dependencies

Many command modules compute behavior and print output in the same function.
That is acceptable for a CLI, but it makes cheap unit tests harder. Some paths
also call `Local::now()` directly, which increases date-sensitive test
fragility.

## Recommended Plan

### Phase 1: Split `commands/task`

Create a directory module:

```text
src/commands/task/
  mod.rs          # public run() and high-level dispatch
  list.rs         # task list, projects, tags
  agenda.rs       # existing agenda submodule
  todo.rs         # existing PKMS todo listing submodule
  shortcuts.rs    # today, week, overdue, upcoming, inbox routing
  mutate.rs       # state, done, postpone, schedule, deadline, add
  id_command.rs   # pkms task <ID> <subcommand> compatibility parsing
  providers.rs    # provider orchestration and source selection
  render.rs       # task JSON/NDJSON/text rendering helpers
```

Keep `pub fn run(config, ctx, command)` in `mod.rs`. Move internal helpers
incrementally and keep visibility narrow with `pub(super)` where needed.

The first extraction should be `id_command.rs` because it is self-contained and
has little business logic. Then move provider orchestration and rendering.

Phase 1 progress:

- [x] Extract task ID-first parsing and dispatch adapter into
  `commands/task/id_command.rs`.
- [x] Extract provider orchestration and source selection into
  `commands/task/providers.rs`.
- [x] Extract task rendering helpers into `commands/task/render.rs`.

### Phase 2: Split `serve.rs`

Replace `src/commands/serve.rs` with a directory module:

```text
src/commands/serve/
  mod.rs          # public ServeOptions + run()
  http.rs         # ServeState, HttpResponse, routing, headers, query parsing
  page.rs         # render_note_html, render_preview_html, panels, page_css/js
  org_html.rs     # render_org_body, blocks, lists, tables, headings
  inline.rs       # render_inline, links, markup, math, escaping, percent codec
  highlight.rs    # syntect setup and code highlighting
  assets.rs       # existing asset helpers
```

Keep the public surface small:

- `ServeOptions`
- `run`

Use `pub(super)` for cross-module helpers. Avoid making renderer internals part
of the broader crate API.

Move tests next to the module they exercise:

- HTTP method/header/query tests in `http.rs`
- page shell, CSS, JS, contents panel, backlinks panel tests in `page.rs`
- org block/list/table/heading tests in `org_html.rs`
- inline formatting, math, escaping, percent codec tests in `inline.rs`
- syntax highlighting tests in `highlight.rs`

Keep `tests/integration/serve.rs` as the end-to-end server contract.

Phase 2 progress:

- [x] Extract serve HTTP routing and response writing into
  `commands/serve/http.rs`.
- [x] Extract serve inline rendering and codec helpers into
  `commands/serve/inline.rs`.
- [x] Extract serve page shell and panels into `commands/serve/page.rs`.
- [x] Extract serve org body rendering into `commands/serve/org_html.rs`.
- [x] Extract serve syntax highlighting into `commands/serve/highlight.rs`.

### Phase 3: Add Pure Command Execution Helpers

For command modules touched during feature work, move toward this shape:

```rust
fn execute(input...) -> Result<CommandOutput>
fn render(ctx: &OutputContext, output: CommandOutput) -> Result<()>
pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &Options) -> Result<()> {
    let output = execute(...)?;
    render(ctx, output)
}
```

Do this incrementally. Good first candidates:

- `query`
- `get`
- `path`
- task list
- serve renderers

This keeps the CLI simple while making behavior testable without spawning the
binary for every branch.

Phase 3 progress:

- [x] Convert `path` to the reference `execute`/`render` shape and add focused
  text-rendering unit tests.

### Phase 4: Use `Workspace` Consistently

Commands that need both parsed files and graph data should prefer
`Workspace::load(config)` over repeated `Graph::load(config)` and independent
file scans.

This preserves the stateless model because the workspace is still per command
invocation. It only avoids duplicate work inside one invocation.

Phase 4 progress:

- [x] Reuse parsed graph scan results in `stats --todos` instead of rereading
  and reparsing note files.

### Phase 5: Isolate Time

Task behavior should accept an explicit `today: NaiveDate` in core logic.
Production command entry points can still use `Local::now().date_naive()`.

Prefer patterns like:

```rust
fn agenda_items_for_on(config: &ResolvedConfig, view: AgendaView, today: NaiveDate) -> Result<Vec<TaskItem>>
```

This reduces flaky or date-sensitive tests and simplifies edge-case coverage for
today, week, overdue, and upcoming behavior.

Phase 5 progress:

- [x] Thread one captured `today` value through `task agenda` via `run_on(...)`
  instead of calling `Local::now()` in multiple agenda branches.

### Phase 6: Improve Test Helpers

Keep the current binary integration suite. It is valuable because `pkms` is a
CLI and pipeline contracts matter.

Add small helpers that reduce setup cost:

- `ResolvedConfig::for_test_db(path)` or a test-only config builder.
- `TestDb::write_note(title, uuid, body)`.
- JSON assertion helpers for common shapes.
- A focused Todoist mock server helper module instead of keeping socket setup in
  large task tests.
- Optional command-level test helpers that call pure `execute` functions
  directly.

Avoid a large test DSL. The goal is less repeated setup, not another framework.

## Testability Strategy

Use three layers of tests:

1. Unit tests for parser, graph, filters, renderers, codecs, and pure command
   execution.
2. Focused integration tests for individual command behavior.
3. Broad binary integration tests for CLI shape, JSON/NDJSON contracts, and
   pipelines.

Prefer adding unit tests under the module that owns the behavior. Use binary
tests when the behavior depends on clap parsing, stdout/stderr, exit codes,
stdin detection, or process-level environment.

## Integration Boundary Guidance

The task provider boundary is a good direction. Keep `TaskItem` as the
source-neutral contract for task integrations.

For Todoist and future integrations, keep these responsibilities separate:

- HTTP client: request/response only.
- DTO mapping: external API models to `TaskItem`.
- Provider: applies `TaskQuery` and source-specific query behavior.

This makes new integrations easier to add and easier to test without real
network calls.

## What Not To Do

Do not:

- rewrite the command system around a registry or framework
- introduce persistent indexes or caches
- add async runtime machinery for the current single-run CLI shape
- make every command generic over writers, clocks, file systems, and clients
- move all tests away from binary integration tests

The current explicit architecture is a strength. The right improvement path is
targeted extraction around modules that have become too dense.

## Suggested First Pull Requests

- [x] Extract task ID-first parsing from `commands/task/mod.rs` into
   `commands/task/id_command.rs`.
- [x] Extract serve HTTP routing and response writing into
   `commands/serve/http.rs`.
- [x] Extract serve inline rendering and codec helpers into
  `commands/serve/inline.rs`.
- [ ] Add a small test config builder used by unit tests that need
   `ResolvedConfig`.
- [x] Convert one narrow command, `path`, to an `execute`/`render` shape as the
  reference pattern.
