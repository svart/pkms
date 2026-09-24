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
`crates/pkms/tests/integration/`.

Use the existing small fixtures instead of open-coded setup when they fit:
`ResolvedConfig::for_test_db(...)` is available for unit tests, and the
integration suite has a chainable `TestDb` builder for notes and tasks.

## Fast Pre-Commit Gate

Before committing or handing work off, run the fast full-feature gate:

```bash
cargo fmt --all -- --check
scripts/check-crate-boundaries.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

This gate covers the complete feature set in a single pass. It catches
formatting, all-feature and default-feature lint, unit tests, integration
tests, and all-feature builds without making every local commit wait on the
full feature-by-feature matrix. The default-feature clippy run catches imports
and helpers used only behind a feature flag.

Also run focused commands for the files you changed. Examples:

```bash
cargo test parser
cargo test task
cargo test --test integration task
```

Use the feature-specific checks below during implementation when they provide a
faster focused loop. They are not additional required pre-commit gates; the
full-feature pre-commit gate above is the normal local gate before committing.

## Feature-Specific Checks

These commands are useful for focused local debugging or for shortening the
edit-test loop while working inside one feature area. Run the relevant focused
check when it helps, then use the [Fast Pre-Commit Gate](#fast-pre-commit-gate)
before committing.

Web viewer or rendered HTML changes:

```bash
cargo clippy --features web -- -D warnings
cargo test --features web <test-name>
cargo test --features web --test integration <test-name>
cargo build --features web
```

SSH file-link check changes:

```bash
cargo clippy --features ssh -- -D warnings
cargo test --features ssh <test-name>
cargo test --features ssh --test integration <test-name>
cargo build --features ssh
```

RAG retrieval changes:

```bash
cargo clippy --features rag -- -D warnings
cargo test --features rag <test-name>
cargo test --features rag --test integration <test-name>
cargo build --features rag
```

An optional ignored live SSH check can verify a real server without making the
normal local gate depend on external network state:

```bash
PKMS_TEST_SSH_TARGET='user@example.org#22' \
PKMS_TEST_SSH_PATH=/absolute/path/that/exists \
PKMS_TEST_SSH_MISSING_PATH=/absolute/path/that/does/not/exist \
cargo test --features ssh test_check_remote_file_links_live_ssh -- --ignored
```

The live check uses strict `~/.ssh/known_hosts` verification and automatic
passwordless public-key auth from standard identity files or the SSH agent,
matching normal `check --remote-file-links` behavior.

When feature interactions are relevant during implementation, prefer an
explicit combined-feature build:

```bash
cargo build --features web,ssh,rag
cargo build --all-features
```

## Project Structure

```text
Cargo.toml                  # virtual workspace root
scripts/
  check-crate-boundaries.sh # workspace dependency boundary check
crates/pkms/                # umbrella binary crate
  src/main.rs               # thin binary wrapper
  src/lib.rs                # umbrella library surface used by tests
  src/runner.rs             # CommandContext construction and command dispatch
  src/command_context.rs    # shared resolved config/output access
  src/app.rs                # app construction and error formatting
  src/cli.rs                # clap derive structs and Command enum
  src/config/               # config loading, db_root resolution, defaults, paths, columns
  src/logging.rs            # PKMS_LOG and PKMS_LOG_FORMAT setup
  src/commands/             # thin command wrappers and output rendering
  src/commands/task/        # task CLI planning/orchestration/rendering
  src/input.rs              # target/stdin/date/column parsing helpers
  src/output.rs             # OutputContext, columns, and structured output helpers
  src/output/               # task tables and terminal markup
  tests/integration/        # binary-level integration tests with mock databases
crates/pkms-org/            # org discovery, parsing, graph, snapshots, org edits
crates/pkms-tokens/         # token encoding/counting leaf utilities
crates/pkms-db/             # note database command logic and link checks
crates/pkms-rag/            # local retrieval, SQLite index, embeddings, RAG API
crates/pkms-task/           # local task domain logic, providers, and mutations
crates/pkms-web/            # local HTTP viewer, HTML rendering, assets, fonts
docs/                       # detailed user, agent, architecture, scenario, and crate docs
  index.md                  # documentation map
  architecture.md           # workspace architecture and data flows
  crates/                   # one architecture page per workspace crate
skills/                     # agent skills for note and pkms workflows
```

Dependency direction is intentionally one-way: `pkms` may depend on all domain
crates; `pkms-db`, `pkms-rag`, `pkms-task`, and `pkms-web` may depend on
`pkms-org`; `pkms-db` and `pkms-rag` may also depend on the leaf `pkms-tokens`
crate. Domain crates must not depend on peer domain crates or on the umbrella
`pkms` crate, and `pkms-tokens` must not depend on any domain crate, unless
`scripts/check-crate-boundaries.sh` explicitly allows the edge. Run the script
after changing manifests. It checks all-feature transitive dependency trees,
not only direct manifest entries.

## Adding or Changing Commands

Follow the existing command shape:

1. Define CLI args in `crates/pkms/src/cli.rs`.
2. Dispatch from `crates/pkms/src/runner.rs`.
3. Put CLI wiring and output rendering in `crates/pkms/src/commands/<name>.rs`,
   or in `crates/pkms/src/commands/<name>/` when the command is a namespace with
   subcommands.
4. Use option structs for command input when more than trivial args are needed.
5. Accept `&CommandContext` for shared resolved config and output access.
6. Load the graph only when the command needs graph data. Use
   `Graph::load_from(scan, links)`, or `OrgSnapshot::load(scan, links)` when a
   command needs both parsed files and graph indexes from one scan.
7. Dispatch structured output through `OutputContext` helpers:
   `print_json`, `print_ndjson`, or `print_json_adaptive`.
8. Add or update integration tests under `crates/pkms/tests/integration/`.
9. Update user docs and `skills/pkms-manager/` references when behavior changes.

Commands should return `anyhow::Result`; `check` may return an `ExitCode` to
represent unhealthy database state.

For read-only commands that shape non-trivial output, prefer an internal
`execute(...)` / `render(...)` split:

```rust
fn execute(config: &ResolvedConfig, opts: &Options) -> Result<CommandOutput>
fn render(ctx: &OutputContext, output: &CommandOutput) -> Result<()>
```

`execute(...)` should own graph/snapshot loading, file reads, filtering,
sorting, limiting, and typed output shaping. `render(...)` should only choose
text, JSON, or NDJSON presentation; text formatting should usually be a pure
`render_text(...) -> String` helper with focused unit tests. Do not force this
shape onto side-effect-first commands such as `open`, `new`, `fix`, task
mutations, or the long-running `serve` command unless a concrete change makes
the split useful.

Command domain behavior should live in the focused crates when possible:
`pkms-org` for org syntax, graph, snapshots, and raw org edits; `pkms-db` for
note database commands; `pkms-task` for task workflows; `pkms-rag` for
retrieval indexing/search/API behavior; and `pkms-web` for the local viewer. The
umbrella `pkms` crate should keep CLI parsing, config mapping, output
formatting, and cross-domain orchestration.

Task command boundaries are strict because local org edits mutate user data:

- `pkms-org` owns org task syntax, typed task insertion, daily note creation,
  and raw org file writes.
- `pkms-task` owns canonical task IDs, filtering, local provider collection,
  task mutations, and typed requests into `pkms-org`.
- `pkms` task command modules own CLI argument adapters, config mapping, output
  rendering, and cross-domain show/open dispatch. They should not format raw org
  task text directly.

The web viewer keeps its public command entry point in
`crates/pkms/src/commands/serve.rs`, but HTTP routing, static/font assets, page
shell, org body rendering, inline markup, KaTeX, and syntax highlighting live in
`crates/pkms-web/src/`.

## Output Contracts

User-facing command output should support `--output-format json` unless a
command explicitly documents a text-only contract, such as `pkms rag index`.
Streamable commands should support `ndjson` where practical. Output structs
should derive `serde::Serialize`.

NDJSON producers emit one JSON object per line, usually with a `uuid` field.
Consumers read targets from stdin via automatic pipe detection or
`--from-stdin`.

Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`,
`mentions`, `rag search`, `rag retrieve`.

Consumers: `get`, `suggest`, `validate`, `task list`.

Keep producer and consumer contracts compatible when changing structured output.
Update schemas under `skills/pkms-manager/schemas/` when JSON output changes.

## Task-System Changes

TODO headings receive deterministic global IDs shared by `task list`,
`task agenda`, and ID-first task actions such as `task <ID> show` and
`task <ID> open`. IDs are based on task status grouping and stable
file/heading ordering within the parsed database: open tasks before closed
tasks; timestamped files (`YYYYMMDDHHMMSS-rest.org`) newest to oldest; equal
timestamps by the rest-of-name; daily files (`YYYY-MM-DD.org`) participate as
`YYYY-MM-DD 00:00:00`; non-conforming files last by path; then heading line
number. Do not use mutable task properties such as priority, deadline,
scheduled date, tags, or project, and do not use clock-relative concepts such
as today, overdue, or upcoming in canonical ID assignment. Filtered views can
show non-contiguous IDs because excluded tasks still occupy their global
positions.

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

## Documentation and Release Workflow

Documentation boundaries:

- `README.md`: short project overview, quick start, core examples, links.
- `AGENTS.md`: compact agent routing guide that points to detailed docs.
- `docs/index.md`: documentation map and first stop after root docs.
- `docs/architecture.md`: runtime shape, crate boundaries, command flow, data
  flows, feature flags.
- `docs/crates/`: one implementation-oriented architecture page per workspace
  crate.
- `docs/scenarios.md`: scenario routing for note database commands, task
  commands, web, RAG, and pipelines.
- `docs/`: detailed installation, configuration, commands, note database
  commands, database format, TODO/agenda behavior, web, RAG, pipelining,
  JSON/NDJSON, workflows, development.
- `skills/pkms-manager/`: agent workflow knowledge for operating `pkms`.

When changing CLI behavior, update the command reference and any affected
workflow docs. When changing agent workflows, update the relevant skill.
When changing crate responsibilities or dependency boundaries, update
[Architecture](architecture.md) and the affected page under `docs/crates/`.

Only commit when the user asks for a commit. Before committing:

- Check whether `AGENTS.md`, `README.md`, `docs/`, and `skills/` need updates.
- Keep `README.md` concise; put detailed usage in `docs/`.
- Update `skills/pkms-manager/` only when CLI behavior or agent workflows
  change.
- Run the [Fast pre-commit gate](#fast-pre-commit-gate).

Before release-ready version commits:

- Update the package version in `Cargo.toml`.
- Update the corresponding `Cargo.lock` package entry.
- Run the [Fast pre-commit gate](#fast-pre-commit-gate).
