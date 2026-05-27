# Execute/Render Conversion Plan

This plan tracks the remaining commands where an `execute(...)` / `render(...)`
split is likely to simplify future development and testing. It intentionally
does not include side-effect-first mutation commands such as `open`, `new`,
`fix`, or task mutation commands.

## Goals

- Keep command behavior identical while reducing coupling between graph access,
  output shaping, and terminal printing.
- Make read-only command logic testable without spawning the binary.
- Keep binary integration tests as CLI contract tests, and add focused unit
  tests for typed outputs and text rendering.
- Avoid adding abstractions where a small helper is enough.

## Target Shape

For read-only commands, prefer this shape when it reduces duplication:

```rust
pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &Options) -> Result<()> {
    let output = execute(config, opts)?;
    render(ctx, &output)
}
```

`execute(...)` should do graph loading, file reads, filtering, sorting, limiting,
and output-shaping. `render(...)` should only decide between text, JSON, and
NDJSON presentation. Text rendering should usually be a pure
`render_text(&Output) -> String` helper that can be unit-tested directly.

## Conversion Steps

### Step 1: Convert `check`

**Why:** `check` has the highest payoff. It currently combines graph loading,
issue collection, display-option derivation, JSON shaping, text rendering, and
exit-code selection in one command flow.

**Planned changes:**

- [x] Introduce a typed command result such as:

  ```rust
  pub struct CheckCommandOutput {
      pub output: CheckOutput,
      pub exit_code: ExitCode,
  }
  ```

- [x] Move issue collection into `execute(config, opts) -> Result<CheckCommandOutput>`.
- [x] Keep health and exit-code derivation close to the typed `CheckOutput`.
- [x] Convert text rendering to `render_text(&CheckOutput) -> String`.
- [x] Make `render(ctx, &CheckCommandOutput)` responsible only for printing and
  returning the selected exit code.
- [x] Add unit tests for health/exit-code behavior and selected text rendering
  cases.
- [x] Keep existing integration tests as the CLI contract layer.

**Acceptance criteria:**

- [x] JSON output remains structurally compatible.
- [x] Text output remains compatible for existing integration tests.
- [x] Unhealthy checks still return exit code `1`.

### Step 2: Convert `get`

**Why:** `get` already has typed JSON output, but text output still uses a
separate path. That duplicates note resolution, file reads, heading parsing, and
neighbor lookup.

**Planned changes:**

- [ ] Replace the separate text processing path with execution into
  `Vec<GetOutput>`.
- [ ] Ensure `execute(...)` reads content once per target and carries enough
  typed data to render text without `Graph`.
- [ ] Add a text-rendering helper that formats `GetOutput`.
- [ ] Keep adaptive JSON behavior for one target versus multiple targets.
- [ ] Add unit tests for rendering content, headings, and links from typed
  output.

**Acceptance criteria:**

- [ ] Text, JSON, and NDJSON formats are driven from the same typed data.
- [ ] `get --headings`, `get --links`, and `get --no-content` behavior remains
  unchanged.

### Step 3: Convert `resolve`

**Why:** `resolve` mixes scanning, filtering, limiting, field selection, query
label construction, and presentation.

**Planned changes:**

- [ ] Move scanning/filtering/limiting/query label construction into
  `execute(config, opts) -> Result<ResolveOutput>`.
- [ ] Keep field filtering as a render-time concern, since it is output-format
  specific.
- [ ] Add `render_text(&ResolveOutput, fields)` and `render_ndjson(...)`
  helpers.
- [ ] Add unit tests for text field selection and query/limit metadata.

**Acceptance criteria:**

- [ ] Existing `--fields` behavior remains compatible.
- [ ] JSON output continues to include the full `ResolveOutput`.
- [ ] NDJSON field filtering remains compatible.

### Step 4: Convert `query`

**Why:** `query` is already close to the target shape. Search and limit handling
can return `QueryOutput`, leaving output formatting behind a small render
function.

**Planned changes:**

- [ ] Move text/embedding search, TODO filtering, and limit handling into
  `execute(config, opts) -> Result<QueryOutput>`.
- [ ] Convert `print_query_output(...)` into `render(ctx, &QueryOutput)`.
- [ ] Extract a pure `render_text(&QueryOutput) -> String`.
- [ ] Add unit tests for result counts, `showed`, and text rendering.

**Acceptance criteria:**

- [ ] Default and `embed` feature builds still pass.
- [ ] Text, JSON, and NDJSON output preserve existing contracts.

### Step 5: Convert `suggest`

**Why:** `suggest` repeats output shaping per format, and it has enough scoring
logic that a typed command output would make future changes safer. The embed
path is feature-gated, so this should follow `query`.

**Planned changes:**

- [ ] Introduce an execution output that can represent one or many targets.
- [ ] Route non-embed suggestion scoring through `execute(...)`.
- [ ] Keep embed support feature-gated, but make it return the same typed output
  where practical.
- [ ] Convert text rendering to a pure helper.
- [ ] Add focused tests for text rendering and target metadata.

**Acceptance criteria:**

- [ ] Non-embed and embed behavior remain compatible.
- [ ] Single-target JSON output keeps its existing shape unless a deliberate
  contract change is documented.

### Step 6: Convert `validate`

**Why:** `validate` already has per-target typed output, but text rendering still
does extra graph-dependent work. The conversion is useful when adding more
validation details.

**Planned changes:**

- [ ] Make `execute(...)` return `Vec<ValidateOutput>`.
- [ ] Ensure `ValidateOutput` contains all fields needed for text rendering.
- [ ] Convert text rendering to `render_text(&[ValidateOutput]) -> String`.
- [ ] Add unit tests for rendering healthy and unhealthy validation results.

**Acceptance criteria:**

- [ ] JSON and NDJSON output remain compatible.
- [ ] Text output no longer requires direct `Graph` access.

### Step 7: Convert `show`

**Why:** `show` already processes targets into typed output. The main remaining
coupling is text rendering that still needs graph lookups for link titles.

**Planned changes:**

- [ ] Add resolved link labels to `ShowOutput` or a text-specific typed child
  structure during execution.
- [ ] Convert text rendering to use only typed output.
- [ ] Add unit tests for rendering parent chain, subtasks, outgoing links, and
  content.

**Acceptance criteria:**

- [ ] Text output remains compatible.
- [ ] JSON output remains compatible unless new fields are skipped or explicitly
  documented.

### Step 8: Light cleanup for `context`

**Why:** `context` already builds typed output. A full conversion is low value,
but the remaining rendering block can be made consistent with the other
commands.

**Planned changes:**

- [ ] Rename or expose the current builder as `execute(...)`.
- [ ] Add a small `render(ctx, &[ContextOutput])` wrapper.
- [ ] Preserve the stderr token summary for text output.

**Acceptance criteria:**

- [ ] Existing context text and stderr behavior remains compatible.
- [ ] JSON and NDJSON output remain compatible.

## Task List and Agenda Refactor Plan

Task list and agenda commands should not be forced into a simple
`execute(...)` / `render(...)` conversion yet. They already have several
special cases:

- PKMS-only fast paths for legacy table behavior.
- Source-neutral provider paths for PKMS and Todoist.
- Shortcut commands such as today, week, overdue, and upcoming.
- Table column resolution that differs by source and view.
- Stdin scope handling that only applies to PKMS.
- Clock-sensitive date filtering and agenda grouping.

The better improvement is to extract smaller, purpose-specific execution helpers
inside the task module.

### Step A: Introduce task list request planning

**Planned changes:**

- [ ] Add a small internal request type that represents the parsed task list
  request after CLI args and raw filters have been validated.
- [ ] Include source selection, criteria, scope, sort, limit, grouping, columns,
  and clock in the request.
- [ ] Keep validation errors near request construction.

**Benefit:** The command runner stops being responsible for remembering which
combinations are legal.

### Step B: Extract task list execution

**Planned changes:**

- [ ] Add an internal helper such as `execute_task_list(config, request)`.
- [ ] Return typed rows plus display metadata such as source, limit, and columns.
- [ ] Keep provider collection, criteria application, sorting, and limiting in
  this execution helper.
- [ ] Preserve the current PKMS fast path until the typed output can represent
  all required table behavior.

**Benefit:** Tests can cover task list source/filter/sort behavior without
capturing stdout.

### Step C: Extract agenda request planning

**Planned changes:**

- [ ] Normalize `task agenda`, `task agenda today`, `week`, `overdue`, and
  `upcoming` into an internal agenda request.
- [ ] Carry `TaskClock` through the request.
- [ ] Keep shortcut-specific defaults explicit: view, date window, sort, and
  default columns.

**Benefit:** Date-sensitive behavior becomes easier to test and reason about.

### Step D: Extract agenda execution

**Planned changes:**

- [ ] Add an internal helper such as `execute_task_agenda(config, request)`.
- [ ] Return typed agenda rows plus source, limit, columns, and grouping date.
- [ ] Keep agenda grouping and table rendering in `commands/task/render.rs`.
- [ ] Add focused tests for today/week/overdue/upcoming request behavior and
  source-neutral filtering.

**Benefit:** Agenda behavior gets the same testability benefits as
execute/render without flattening every task command into one abstraction.

### Step E: Keep rendering centralized

**Planned changes:**

- [ ] Keep terminal, JSON, and NDJSON rendering in `commands/task/render.rs`.
- [ ] Let list and agenda runners call rendering with typed execution output.
- [ ] Avoid moving mutation command output into this flow.

**Benefit:** The task module gains clearer boundaries while preserving the
existing task source and table abstractions.

## Explicit Non-Targets

- [ ] Do not convert task mutation commands just for consistency.
- [ ] Do not convert `open`; it is an editor-launch side effect.
- [ ] Do not convert `new` unless note creation is later split into a reusable
  library workflow.
- [ ] Do not convert `fix` unless broken-link replacement needs richer dry-run
  testing.
- [ ] Do not change JSON or NDJSON contracts without a separate compatibility
  decision.

## Suggested Implementation Order

1. [x] Convert `check`.
2. [ ] Convert `get`.
3. [ ] Convert `resolve`.
4. [ ] Convert `query`.
5. [ ] Convert `suggest`.
6. [ ] Convert `validate`.
7. [ ] Convert `show`.
8. [ ] Apply the light `context` cleanup.
9. [ ] Refactor task list request planning.
10. [ ] Refactor task list execution.
11. [ ] Refactor agenda request planning.
12. [ ] Refactor agenda execution.
13. [ ] Review whether any remaining read-only command still has duplicated
    execution paths.

## Verification

After each step:

- [ ] Run focused unit tests for the touched module.
- [ ] Run the relevant integration test subset.
- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo clippy -- -D warnings`.

After changes touching task list or agenda:

- [ ] Run `cargo test --test integration task`.
- [ ] Run `cargo clippy --features todoist -- -D warnings`.

After changes touching `query` or `suggest` embed behavior:

- [ ] Run `cargo test --features embed`.
