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

4. [ ] Move task provider implementation out of `commands/task/mod.rs`.

   Partially done. `commands/task/id_command.rs`,
   `commands/task/providers.rs`, and `commands/task/render.rs` were extracted.
   PKMS inbox/provider collection now lives in `tasks/pkms.rs`. Todoist
   provider routing and PKMS mutation helpers still need narrower homes.

5. [ ] Inject time where behavior depends on `today`.

   Partially done. `task agenda` now captures one `today` value and threads it
   through `run_on(...)`, but other task paths still call `Local::now()`
   directly.

6. [x] Split `serve.rs` by responsibility.

   Done. The implemented module split is:

   - `commands/serve/http.rs`
   - `commands/serve/assets.rs`
   - `commands/serve/page.rs`
   - `commands/serve/org_html.rs`
   - `commands/serve/inline.rs`
   - `commands/serve/highlight.rs`

7. [ ] Consolidate task filter duplication.

   Not done. `tasks/filter.rs` and older row/filter helpers in
   `commands/task_common.rs` still both exist.

8. [ ] Improve test fixtures with a small builder.

   Partially done. `ResolvedConfig::for_test_db(...)` was added and adopted in
   serve renderer tests. A broader `TestDb::new().note(...).task(...)` fixture
   builder has not been added.

9. [ ] Keep binary tests for contracts, add library tests for logic.

   Partially done. Binary integration tests remain the contract layer, and new
   unit tests were added for `path` rendering. More command logic still relies
   primarily on binary integration tests.

## Suggested Order

1. [x] Add `lib.rs` and keep behavior identical.
2. [ ] Add focused fixture builder in tests.
3. [ ] Extract task provider modules.
4. [ ] Extract task mutation/date logic and inject clock.
5. [x] Split `serve.rs` with move-only commits.
6. [x] Convert one command, `path`, to typed output as the model.
7. [ ] Apply the typed-output pattern as commands are touched for features.

## Not Yet Done

- Move remaining task provider and mutation implementation into narrower
  `tasks/*` modules.
- Consolidate task filtering so source-neutral filters and row-oriented filters
  do not drift.
- Continue replacing scattered `Local::now()` calls in task code with explicit
  dates in core logic.
- Add a focused `TestDb` builder for smaller, clearer tests.
- Move more pure command behavior behind typed `execute`/`render` boundaries
  when those commands are touched for feature work.
