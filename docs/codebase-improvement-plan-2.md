# Codebase Improvement Plan 2 - Status

This file is the status-tracked version of the second improvement plan. It
reflects the implementation work already completed in the recent phase-by-phase
refactor commits.

## Assessment Update

- `src/lib.rs` exists and `src/main.rs` is now a thin binary wrapper around the
  library crate.
- `commands/task/mod.rs` is smaller than it was: ID-first parsing, provider
  orchestration, and rendering helpers have been extracted.
- `serve.rs` has been split by responsibility into HTTP, assets, page
  rendering, org rendering, inline rendering, and syntax highlighting modules.
- Some command behavior is easier to unit test now, especially `path`, which has
  a typed output and text-rendering unit tests.
- Testability has improved incrementally with `ResolvedConfig::for_test_db(...)`
  and by reducing repeated parsing in `stats --todos`.

## Improvement Plan

1. [x] Introduce a `lib.rs` and make `main.rs` a thin binary wrapper.

   Done. The binary initializes logging, parses CLI args, and delegates to
   `pkms::runner::run(...)`.

2. [x] Add a lightweight `CommandContext`.

   Done. `CommandContext` now carries `&ResolvedConfig` and `&OutputContext`,
   exposes `load_graph()` / `load_workspace()`, and is used by the runner and
   the `path` command. Most command entry points still receive config/output
   directly and can migrate incrementally.

3. [x] Split command execution from presentation incrementally.

   Started with `path`: it now has `execute(...) -> Result<PathOutput>`,
   `render(...)`, and focused `render_text(...)` unit tests.

4. [x] Move task provider implementation out of `commands/task/mod.rs`.

   Done. `commands/task/id_command.rs`, `commands/task/providers.rs`, and
   `commands/task/render.rs` were extracted. PKMS inbox/provider collection now
   lives in `tasks/pkms.rs`, org-file PKMS task mutations live in
   `tasks/pkms_mutation.rs`, and Todoist provider API/metadata routing lives in
   `tasks/todoist_provider.rs`.

5. [x] Inject time where behavior depends on `today`.

   Done. Task paths use `TaskClock` captured at the command boundary and pass
   it through task indexing, providers, source-neutral filters, agenda grouping,
   task-add date parsing, daily inbox resolution, and mutation reloads. Direct
   task clock access is centralized in `tasks/clock.rs`.

6. [x] Split `serve.rs` by responsibility.

   Done. The implemented module split is:

   - `commands/serve/http.rs`
   - `commands/serve/assets.rs`
   - `commands/serve/page.rs`
   - `commands/serve/org_html.rs`
   - `commands/serve/inline.rs`
   - `commands/serve/highlight.rs`

7. [x] Consolidate task filter duplication.

   Done. State/tag/type text filters and priority filters are shared through
   `tasks/filter.rs`; the older row-oriented task paths now use the same parser
   and matcher helpers as the source-neutral task filter path.

8. [x] Improve test fixtures with a small builder.

   Done. `ResolvedConfig::for_test_db(...)` was added and adopted in serve
   renderer tests. Integration tests also have a small chainable
   `TestDb::new().note(...).task(...)` fixture builder.

9. [ ] Keep binary tests for contracts, add library tests for logic.

   Partially done. Binary integration tests remain the contract layer, and unit
   tests were added for `path` and `orphans` rendering. More command logic still
   relies primarily on binary integration tests.

## Suggested Order

1. [x] Add `lib.rs` and keep behavior identical.
2. [x] Add focused fixture builder in tests.
3. [x] Extract task provider modules.
4. [x] Extract task mutation/date logic and inject clock.
5. [x] Split `serve.rs` with move-only commits.
6. [x] Convert one command, `path`, to typed output as the model.
7. [ ] Apply the typed-output pattern as commands are touched for features.

## Not Yet Done

- Move more pure command behavior behind typed `execute`/`render` boundaries
  when those commands are touched for feature work; `path` and `orphans` are the
  current examples.
