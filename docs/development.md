# Development

`pkms` is primarily a stateless single-run CLI. Each non-interactive invocation
parses args, resolves the database root once, reads org files from disk,
computes the result, prints, and exits.

Do not introduce persistent caches, daemon processes, or watch mode. In builds
with the `web` feature, `pkms serve` is the intentional exception: it is a
foreground local HTTP viewer that loads the graph at startup and keeps no
persistent derived state. If performance needs improvement, optimize fresh
discovery, parsing, and graph construction.

## Bug Reproduction

When reproducing a reported behavior against an available notes database, build
from the current checkout first:

```bash
cargo build
target/debug/pkms --db <db-root> <command>
```

Do not rely on an installed `pkms` binary unless the report is specifically
about the installed version. If the user provides example files from the notes
database, inspect those files before broader analysis.

For output or parsing bugs, prefer the smallest fixture that shows the issue.
For task bugs, preserve the distinction between local PKMS tasks and Todoist
tasks, because they have different source and mutation paths.

## Local Development Loop

During feature development, run the smallest checks that cover the code you just
changed. Keep the loop fast enough that failures stay close to the edit that
caused them.

For narrow changes, run focused tests first:

```bash
cargo test <test-name>
cargo test --test integration <test-name>
```

For parser, graph, task, output, or command-dispatch behavior, broaden coverage
to the relevant integration tests before moving to the pre-commit gate.

Unit tests live next to module code under `#[cfg(test)]`. Integration tests
spawn `target/debug/pkms` with temporary mock databases from
`tests/integration/`.

Use the existing small fixtures instead of open-coded setup when they fit:
`ResolvedConfig::for_test_db(...)` is available for unit tests, and the
integration suite has a chainable `TestDb` builder for notes and tasks.

## Fast Pre-Commit Gate

Before committing or handing work off for review, run the fast default-feature
gate:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
```

This is intentionally shorter than CI. It catches formatting, default-feature
lint, unit tests, integration tests, and default builds without making every
local commit wait on the full feature matrix.

Also run focused commands for the files you changed. Examples:

```bash
cargo test parser
cargo test task
cargo test --test integration task
```

If the change touches feature-gated code, run the relevant checks from the next
section before committing.

## Feature-Specific Checks

Todoist changes:

```bash
cargo clippy --features todoist -- -D warnings
cargo test --features todoist <test-name>
cargo test --features todoist --test integration <test-name>
cargo build --features todoist
```

Embedding changes:

```bash
cargo build --features embed
cargo build --features embed,todoist
```

Web viewer or rendered HTML changes:

```bash
cargo clippy --features web -- -D warnings
cargo test --features web <test-name>
cargo test --features web --test integration <test-name>
cargo build --features web
```

When feature interactions are relevant, prefer an explicit combined-feature
build:

```bash
cargo build --features embed,web
cargo build --all-features
```

## Full CI Gate

CI should run the comprehensive matrix. Local development should not routinely
wait on this full gate unless preparing a release or investigating CI behavior.

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked
cargo build --locked --features embed
cargo build --locked --features todoist
cargo build --locked --features embed,todoist
cargo build --locked --features web
cargo build --locked --features embed,web
cargo build --locked --all-features
cargo test --locked
cargo test --locked --features todoist
cargo test --locked --features web
cargo test --locked --features todoist,web
cargo run --locked -- --help
cargo run --locked --features todoist -- --help
cargo run --locked --features web -- --help
cargo build --locked --release
```

CI intentionally does not run tests with the `embed` feature enabled. The
feature is covered by build and clippy checks; runtime tests would execute
ONNX Runtime binaries through `fastembed`, which are not compatible with the
current Gitea runner CPU.

The Gitea workflow in `.gitea/workflows/rust.yml` is expected to match this
gate. If the documented gate changes, update CI in the same change.

## Project Structure

```text
src/
  main.rs             # thin binary wrapper: logging, CLI parse, runner call
  lib.rs              # library crate surface used by the binary and tests
  runner.rs           # App setup, CommandContext construction, command dispatch
  command_context.rs  # shared config/output access plus graph/workspace loaders
  app.rs              # App construction, output context, error formatting
  cli.rs              # clap derive structs and Command enum
  config.rs           # config loading, db_root resolution, ResolvedConfig
  logging.rs          # PKMS_LOG and PKMS_LOG_FORMAT setup
  discovery.rs        # recursive .org discovery with ignore patterns
  parser.rs           # org parser for note metadata, links, headings, tasks
  org_date.rs         # org timestamp parser
  org_edit.rs         # local org file editing helpers
  graph/              # in-memory graph build, search, traversal, validation
  commands/           # one module per subcommand; task/ and serve/ own helpers
  commands/task/      # task ID parsing, providers, agenda/todo paths, rendering
  commands/serve/     # HTTP, assets, page, org HTML, inline, highlighting
  tasks/              # source-neutral task model, filters, providers, mutations
  input.rs            # target/stdin/date/column parsing helpers
  output.rs           # OutputContext and output format helpers
  output/table.rs     # adaptive table layout
  corpus.rs           # text corpus helpers
  tokens.rs           # token counting and truncation
  util.rs             # small shared utility helpers
  workspace.rs        # workspace/path helpers
tests/integration/    # binary-level integration tests with mock databases
docs/                 # detailed user and contributor docs
skills/               # Codex skills for note and pkms workflows
.gitea/workflows/     # CI and release automation
```

## Adding or Changing Commands

Follow the existing command shape:

1. Define CLI args in `src/cli.rs`.
2. Dispatch from `src/runner.rs`.
3. Put behavior in `src/commands/<name>.rs`, or in `src/commands/<name>/` when
   the command is a namespace with subcommands.
4. Use option structs for command input when more than trivial args are needed.
5. Accept `&ResolvedConfig` and `&OutputContext`, or `&CommandContext` when the
   command benefits from shared graph/workspace loader helpers.
6. Load the graph only when the command needs graph data. Use
   `Graph::load(config)` or `CommandContext::load_graph()`. Use
   `Workspace::load(config)` or `CommandContext::load_workspace()` when a
   command needs both parsed files and graph data.
7. Dispatch structured output through `OutputContext` helpers:
   `print_json`, `print_ndjson`, or `print_json_adaptive`.
8. Add or update integration tests under `tests/integration/`.
9. Update user docs and `skills/pkms-manager/` references when behavior changes.

Commands should return `anyhow::Result`; `check` may return an `ExitCode` to
represent unhealthy database state.

For read-only commands that shape non-trivial output, prefer an internal
`execute(...)` / `render(...)` split:

```rust
fn execute(config: &ResolvedConfig, opts: &Options) -> Result<CommandOutput>
fn render(ctx: &OutputContext, output: &CommandOutput) -> Result<()>
```

`execute(...)` should own graph/workspace loading, file reads, filtering,
sorting, limiting, and typed output shaping. `render(...)` should only choose
text, JSON, or NDJSON presentation; text formatting should usually be a pure
`render_text(...) -> String` helper with focused unit tests. Do not force this
shape onto side-effect-first commands such as `open`, `new`, `fix`, task
mutations, or the long-running `serve` command unless a concrete change makes
the split useful.

The web viewer keeps its public command entry point in `src/commands/serve.rs`.
Keep responsibility-specific helpers in `src/commands/serve/`: HTTP routing and
responses in `http.rs`, static/font assets in `assets.rs`, page shell and panels
in `page.rs`, org body rendering in `org_html.rs`, inline markup and percent
codec helpers in `inline.rs`, and syntax highlighting in `highlight.rs`.

## Output Contracts

All user-facing command output should support `--output-format json`. Streamable
commands should support `ndjson` where practical. Output structs should derive
`serde::Serialize`.

NDJSON producers emit one JSON object per line, usually with a `uuid` field.
Consumers read targets from stdin via automatic pipe detection or
`--from-stdin`.

Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`.

Consumers: `get`, `suggest`, `validate`, `context`, `task list`.

Keep producer and consumer contracts compatible when changing structured output.
Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.

## Task-System Changes

TODO headings receive deterministic global IDs shared by `task list`,
`task agenda`, and ID-first task actions such as `task p<ID> show` and
`task p<ID> open`. IDs are based on task status grouping and stable ordering
within the parsed database: priority, parsed deadline date, parsed scheduled
date, path, and line number. Do not use clock-relative concepts such as today,
overdue, or upcoming in canonical ID assignment. Filtered views can show
non-contiguous IDs because excluded tasks still occupy their global positions.

Use the shared task-index helpers rather than implementing a parallel task ID
scheme.

For task-command changes, also update [Task System Design](task-system.md) when
the source model, ID strategy, mutation boundaries, or non-goals change. Update
[TODO and Agenda](todo-agenda.md), [Commands](commands.md), and
`skills/pkms-manager/references/task.md` for user-visible behavior.

## Heading Nodes

Each org-mode heading with an `:ID:` property is a first-class graph node. Code
that resolves UUIDs, validates duplicate IDs, checks links, or builds
neighborhoods must account for both note-level and heading-level IDs.

## Diagnostics

Use `PKMS_LOG` for local debugging when command output alone is not enough.
Logs are written to stderr so text, JSON, and NDJSON stdout remain parseable.
`PKMS_LOG=1` enables debug logs; module filters such as
`PKMS_LOG=pkms::graph=debug` keep output focused. Add `PKMS_LOG_FORMAT=json`
when logs need to be parsed by tools.

For Todoist API issues, prefer `PKMS_LOG_HTTP=1`. It emits request/response
metadata and pagination counts without logging tokens, request bodies, task
content, or descriptions.

## Documentation and Release Workflow

Documentation boundaries:

- `README.md`: short project overview, quick start, core examples, links.
- `docs/`: detailed installation, configuration, commands, database format,
  TODO/agenda behavior, pipelining, JSON/NDJSON, workflows, development.
- `skills/pkms-manager/`: agent workflow knowledge for operating `pkms`.

When changing CLI behavior, update the command reference and any affected
workflow docs. When changing agent workflows, update the relevant skill.

Only commit when the user asks for a commit. Before committing:

- Check whether `AGENTS.md`, `README.md`, `docs/`, and `skills/` need updates.
- Keep `README.md` concise; put detailed usage in `docs/`.
- Update `skills/pkms-manager/` only when CLI behavior or agent workflows
  change.
- Run the [Fast pre-commit gate](#fast-pre-commit-gate) and relevant
  [Feature-specific checks](#feature-specific-checks).

Before release-ready version commits:

- Update the package version in `Cargo.toml`.
- Update the corresponding `Cargo.lock` package entry.
- Run or confirm the [Full CI gate](#full-ci-gate).
