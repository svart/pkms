# Architecture and Test Maintainability Plan

This document records the current architecture and test-harness review for
`pkms`, plus a plan for hardening the codebase without changing user-facing
behavior.

## Summary

The codebase mostly matches the intended architecture and is in good current
health. It is idiomatic Rust, behavior is covered by fast tests, and the
development documentation gives a clear path for adding commands.

No blocking architecture or coding-standard failures were found. The main
improvement area is future maintainability: the task command layer has become
dense, and a few integration-test harness paths should be made more uniform and
deterministic before larger refactors.

All work from this plan should preserve current CLI behavior, text output,
structured JSON and NDJSON contracts, task ID semantics, and config resolution
rules.

## Current Strengths

- The core CLI architecture is coherent: `main` parses CLI input, `runner`
  dispatches commands, command modules own behavior, and shared output helpers
  preserve structured stdout.
- The project remains aligned with the stateless single-run model: commands load
  files, compute output, print, and exit. `pkms serve` remains the explicit
  foreground web-viewer exception.
- `Graph`, `Corpus`, and `Workspace` provide clear boundaries for fresh scanning,
  parsing, graph construction, and commands that need both raw parsed files and
  graph data.
- `OutputContext` centralizes JSON and NDJSON printing enough to keep parseable
  output predictable.
- The task system has useful source-neutral concepts: `TaskItem`,
  `TaskFilters`, `TaskId`, task providers, and shared task-index helpers.
- The test suite is already strong and fast. It combines focused unit tests with
  binary-level integration tests over temporary note databases.
- Feature-gated surfaces have meaningful coverage. Todoist behavior uses mock
  local HTTP tests, and web-rendering behavior has focused unit and integration
  tests.

## Current Risks

### Task Command Complexity

`src/commands/task/mod.rs` is the main maintainability risk. It currently owns
task dispatch, request planning, source-specific compatibility paths,
source-neutral execution, filtering, sorting, rendering handoff, and mutation
routing.

The existing abstractions are useful, but the branching around PKMS-native
paths, source-neutral paths, Todoist behavior, `source:all`, `--from-stdin`,
`--group`, and configured columns makes the file harder to extend safely than
the rest of the codebase.

This should be addressed incrementally. Do not rewrite the task system
wholesale, and do not change CLI behavior as part of cleanup work.

### Harness Uniformity

Most integration tests run through helpers that isolate `XDG_CONFIG_HOME`, clear
PKMS-related environment variables, and run against temporary databases. Broad
smoke tests should also use those helpers so environment or local config cannot
leak into behavior checks.

### Fixture Granularity

The large shared fixture is valuable for broad regression coverage, but future
tests should prefer focused fixtures when possible. A test that only needs one
note and one task should build exactly that state, so failures reveal the broken
behavior rather than an incidental dependency on the shared fixture.

### Refactor Safety

Before splitting task modules, add characterization tests around the compatibility
boundaries. The highest-risk regressions are not type errors; they are subtle
changes to output shape, default source behavior, task ordering, filter
precedence, and ID-first task actions.

## Recommended Plan

### 1. Harden the Integration Harness

Add or strengthen shared helpers in `tests/integration/`:

- Run broad command smoke tests through the same isolated `XDG_CONFIG_HOME`,
  `PKMS_DB_ROOT`, `PKMS_LOG`, `PKMS_LOG_FORMAT`, and `PKMS_LOG_HTTP` setup used
  by the rest of the harness.
- Add small assertion helpers for JSON object output, NDJSON line output, and
  JSON error output.
- Prefer harness helpers over direct `Command::new(pkms_binary())` in new tests,
  unless the test is specifically about process environment setup.

Expected outcome: integration tests become easier to write and less sensitive to
the developer machine or shell environment.

### 2. Prefer Focused Fixtures for New Tests

Keep `TestDb::fixture()` for broad smoke and regression coverage. For new tests,
prefer:

- `TestDb::clean()` for a blank roam tree.
- Chainable `TestDb` builder methods for one-note or one-task scenarios.
- Explicit fixture content when testing parser, graph, task ID, or output edge
  cases.

Add builder methods only when they remove repeated org boilerplate without
hiding important test setup. Test fixtures should remain descriptive rather than
overly abstract.

Expected outcome: new tests are easier to read, failures are easier to diagnose,
and the large fixture does not become a hidden dependency for unrelated behavior.

### 3. Add Task Characterization Tests Before Refactoring

Before moving task command code, cover these behavior boundaries:

- PKMS-native `task list` text, JSON, and NDJSON output.
- PKMS-native `task agenda` grouped text and structured output.
- Source-neutral task output for `source:pkms`, `source:todoist`, and
  `source:all`.
- `task list --from-stdin` scoping behavior.
- `task list --group` behavior and source restrictions.
- Configured column defaults and explicit `--columns` overrides.
- Todoist `todoist.filter` precedence over default filters and shortcut filters.
- ID-first actions such as `task p<ID> show`, `open`, `state`, `done`,
  `schedule`, `deadline`, and `postpone`.
- Stable canonical PKMS task IDs when filters exclude earlier tasks.

Expected outcome: task refactors can be reviewed as structure-only changes with
high confidence that current behavior is preserved.

### 4. Split Task Command Responsibilities Incrementally

Keep the public command entry point stable:

```rust
commands::task::run(config, ctx, command)
```

Move internals only when tests already characterize the affected behavior. A
reasonable target structure is:

- `commands/task/plan.rs`: parse task command args into request structs and
  choose PKMS-native versus source-neutral execution paths.
- `commands/task/providers.rs`: keep provider collection and metadata
  composition.
- `commands/task/render.rs`: keep task table, JSON, NDJSON, mutation, and
  metadata rendering.
- `commands/task/mutations.rs`: route add, state, done, schedule, deadline, and
  postpone operations.
- `commands/task/mod.rs`: remain a small dispatch facade.

Do not introduce a framework or a generic command pipeline. The goal is to make
the existing responsibilities easier to find and test, not to generalize command
execution.

Expected outcome: future task features have an obvious place to go, and review
surface area is smaller.

### 5. Keep Existing Useful Abstractions

Do not remove these boundaries without a concrete replacement and regression
coverage:

- `Graph` for parsed note and heading graph behavior.
- `Corpus` for scanned and parsed file results.
- `Workspace` for commands needing both corpus and graph.
- `OutputContext` for structured output.
- `TaskItem`, `TaskId`, `TaskFilters`, and task provider traits for
  source-neutral task behavior.
- Shared task-index helpers for canonical PKMS task IDs.

Add new abstractions only when they remove repeated behavior, reduce real
branching complexity, or create a clearer test boundary.

Expected outcome: the codebase stays concrete and idiomatic instead of drifting
into generalized infrastructure.

### 6. Keep Command Additions Predictable

For each new command or user-visible feature:

1. Define CLI args in `src/cli.rs`.
2. Dispatch from `src/runner.rs`.
3. Put behavior in the appropriate `src/commands/` module.
4. Use typed option and output structs for non-trivial behavior.
5. Support `--output-format json`; support NDJSON for streamable output where
   practical.
6. Add focused unit tests for pure shaping/rendering logic.
7. Add binary-level integration tests under `tests/integration/`.
8. Update user docs and `skills/pkms-manager/` references when behavior changes.
9. Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.

Expected outcome: new features remain easy to integrate and hard to regress.

## Verification Baseline

The review baseline passed these checks:

```bash
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Use this baseline when starting behavior-preserving refactors from this plan.
