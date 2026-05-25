# AGENTS.md — pkms Project Guide for AI Agents

This file is the operating guide for coding agents working in this repository.
Keep it focused on agent behavior, project invariants, and implementation
workflow. User-facing documentation belongs in `README.md` and `docs/`.

## First Response to Bug Reports

When a user reports behavior of the tool against an available notes database,
reproduce it first with the binary built from the current checkout:

```bash
cargo build
target/debug/pkms --db <db-root> <command>
```

Do not rely on an installed `pkms` binary unless the user explicitly asks to
debug the installed version. If the user provides example files from the notes
database, inspect those files before broader analysis.

## Build, Lint, and Test

After code or documentation changes, run these commands in this strict order:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo build --features=embed
cargo test
cargo test --test integration
cargo run -- --help
```

All checks must pass. Fix clippy warnings directly; do not suppress clippy lints
unless the user explicitly instructs otherwise.

## Committing Changes

Only commit when the user asks for a commit. Before committing:

- Check whether `AGENTS.md`, `README.md`, `docs/`, and `skills/` need updates.
- Keep `README.md` concise; put detailed usage in `docs/`.
- Update `skills/pkms-manager/` only when CLI behavior or agent workflows change.
- Update the package version in `Cargo.toml` and the corresponding `Cargo.lock`
  entry.
- Run the full verification sequence above.

## Project Structure

```text
src/
  main.rs             # CLI parse, App setup, command dispatch
  app.rs              # App construction, output context, error formatting
  cli.rs              # clap derive structs and Command enum
  config.rs           # config loading, db_root resolution, ResolvedConfig
  discovery.rs        # recursive .org discovery with ignore patterns
  parser.rs           # org parser for note metadata, links, headings, tasks
  org_date.rs         # org timestamp parser
  graph/              # in-memory graph build, search, traversal, validation
  commands/           # one module per subcommand; task/ owns task subcommands
  input.rs            # target/stdin/date/column parsing helpers
  output.rs           # OutputContext and output format helpers
  output/table.rs     # adaptive table layout
  corpus.rs           # text corpus helpers
  tokens.rs           # token counting and truncation
  workspace.rs        # workspace/path helpers
tests/integration/    # binary-level integration tests with mock databases
docs/                 # detailed user and contributor docs
skills/               # Codex skills for note and pkms workflows
```

Prefer checking the current source over trusting this outline when a file has
moved or behavior has changed.

## Architecture Invariants

`pkms` is a stateless single-run CLI:

- Parse args, resolve config, load org files, compute, print, exit.
- Do not introduce persistent caches, databases, daemons, watch mode, or server
  mode.
- `Graph::load()` re-scans and re-parses `.org` files for each invocation.
- `Config::load()` reads optional user config; `Config::resolve()` produces a
  `ResolvedConfig`.
- `db_root` resolution happens once through CLI `--db`, then `PKMS_DB_ROOT`, then
  config file `db_root`.
- Command implementations read paths from `ResolvedConfig`, usually through
  `resolved_db_root()`, `resolve_new_notes_dir()`, or `resolve_ignore_patterns()`.

If performance needs improvement, optimize the fresh discovery, parse, and graph
construction path. Do not persist derived state between invocations.

## Command Pattern

When adding or changing commands, follow the existing shape:

1. Define CLI args in `src/cli.rs`.
2. Dispatch from `src/main.rs`.
3. Put behavior in `src/commands/<name>.rs`, or in `src/commands/<name>/`
   when the command is a namespace with subcommands.
4. Use option structs for command input when more than trivial args are needed.
5. Accept `&ResolvedConfig` and `&OutputContext`.
6. Load the graph with `Graph::load(config)` only when the command needs graph
   data.
7. Dispatch structured output through `OutputContext` helpers:
   `print_json`, `print_ndjson`, or `print_json_adaptive`.
8. Add or update integration tests under `tests/integration/`.

Commands should return `anyhow::Result`; `check` may return an `ExitCode` to
represent unhealthy database state.

## Code Conventions

- Use `anyhow::Result` for fallible functions. Do not add custom error types
  without a strong local reason.
- Derive `serde::Serialize` for command output structs.
- Every command should support `--output-format json`; stream-like commands
  should support `ndjson` when practical.
- Prefer shared parsing helpers in `input.rs` and output helpers in `output.rs`.
- Keep comments sparse and useful. Add them for non-obvious logic, not for
  restating code.
- Preserve the stateless model even when optimizing.

## Canonical Task IDs

TODO headings receive deterministic global IDs shared by `todo`, `agenda`,
`show`, and `open`. IDs are based on task status grouping and stable ordering
within the parsed database. Filtered views can show non-contiguous IDs because
excluded tasks still occupy their global positions.

Use the shared task-index helpers rather than implementing a parallel task ID
scheme.

## Heading Nodes

Each org-mode heading with an `:ID:` property is a first-class graph node. Code
that resolves UUIDs, validates duplicate IDs, checks links, or builds
neighborhoods must account for both note-level and heading-level IDs.

## Testing Patterns

- Unit tests live next to module code under `#[cfg(test)]`.
- Integration tests spawn `target/debug/pkms` with temporary mock databases.
- Property-based tests cover parser and graph panic resistance.
- JSON support is guarded by the all-commands JSON integration test.
- Pipeline behavior is tested with NDJSON producer/consumer integration tests.
- Mock database helpers live in `tests/integration/mod.rs`.

For a narrow change, add focused tests near the changed behavior. For shared
parsing, graph, task, output, or command-dispatch behavior, broaden coverage to
the relevant integration tests.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `embed` | off | Enables embedding-based `query` and `suggest` behavior through `fastembed`. |

Always verify both default and `embed` builds.

## Command Pipelining

NDJSON producers emit one JSON object per line, usually with a `uuid` field.
Consumers read targets from stdin via automatic pipe detection or `--from-stdin`.

- Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`
- Consumers: `get`, `suggest`, `validate`, `context`, `todo`, `show`

Keep producer and consumer contracts compatible when changing structured output.

## Documentation Boundaries

- `README.md`: short project overview, quick start, core examples, links.
- `docs/`: detailed installation, configuration, commands, database format,
  TODO/agenda behavior, pipelining, JSON/NDJSON, workflows, development.
- `skills/pkms-manager/`: agent workflow knowledge for operating `pkms`.

When changing CLI behavior, update the command reference and any affected
workflow docs. When changing agent workflows, update the relevant skill.
