# AGENTS.md — pkms project guide for AI agents

If user reports problems about work of the tool with currently available
database, always first reproduce the behavior using binary built from latest
version of code in this repository. 

If user gives some example files from the notes database always check them first
before doing analysis.

## Build, Lint & Test

After changes, run these commands **in this strict order**:

```bash
cargo fmt --check              # 1. Check formatting (fail if unformatted)
cargo clippy -- -D warnings    # 2. Lint with clippy (deny all warnings)
cargo build                    # 3. Build the binary
cargo build --features=embed   # 4. Build the binary
cargo test                     # 5. Run all unit + integration tests (239+ tests, ~1s)
cargo test --test integration  # 6. Integration tests only (mock DB)
target/debug/pkms --help       # 7. Verify CLI works
```

IMPORTANT: All tests **MUST** pass. Fix all issues which appear when running these commands.
IMPORTANT: Never suppress clippy lints by yourself. Always fix issues, mentioned by clippy.

## Committing changes

When you are done with changes, before committing the work into git do next:
- check that AGENTS.md, README.md and SKILL.md has necessary information, if not update them accordingly;
- update the version of the package in Cargo.toml.

Then commit changes.

## Project structure

```
src/
  main.rs           # CLI dispatch — match on Command enum, call commands::*::run()
  cli.rs            # clap derive structs — Cli, Command enum (all subcommands)
  config.rs         # ~/.config/pkms.toml loading, resolved_db_root() accessor
  discovery.rs      # Recursive .org file discovery with ignore patterns
  org_date.rs       # Org-mode timestamp parser (SCHEDULED/DEADLINE dates)
  parser.rs         # org-mode parser: IDs, titles, filetags, aliases, refs, links, headings, priorities, SCHEDULED/DEADLINE
  util.rs           # Shared helpers: path_string, is_stdin_piped, read_stdin_ndjson
  output.rs         # OutputContext: format dispatch (Text/Json/Ndjson), print helpers
  graph/            # In-memory graph module (split into submodules)
  commands/         # One file per subcommand
tests/
  integration/      # Integration tests for commands and flags
```

## How to add a new command

1. **`src/cli.rs`** — Add variant to `Command` enum with clap attributes
2. **`src/commands/<name>.rs`** — Create file with `pub fn run(...)` that accepts `&Config, &OutputContext, ...` and returns `anyhow::Result<()>`
3. **`src/commands/mod.rs`** — Add `pub mod <name>;`
4. **`src/main.rs`** — Add `Command::<Name> => commands::<name>::run(...)` arm
5. **`tests/integration/<name>.rs`** — Create test file with `use super::*;` and `#[test]` functions

## Architecture: stateless single-run model

`pkms` is designed as a pure CLI pipeline — parse args, load org files from disk, compute, print, exit.
There is **no state persisted between invocations**:
- No cache files, databases, or daemon processes.
- No server mode, watch mode, or background workers.
- `Graph::load()` re-scans and re-parses every `.org` file on each invocation.
- Every command's `run()` is a self-contained function.
- **Single resolution point**: `db_root` is resolved once in `main.rs` from CLI `--db` > `PKMS_DB_ROOT` env var > `config.db_root`, then stored into `Config` via `cfg.db_root = Some(resolved)`. All functions read from `config.resolved_db_root()?`.

**Do not introduce** statefulness (caches, databases, daemon mode) in future development.
If performance optimization is needed, optimize reading from scratch on every run — do not persist state.

## Code conventions

- **No comments** unless the logic is non-obvious. Code should be self-documenting.
- **`anyhow::Result`** for all fallible functions. No custom error types.
- **`serde::Serialize`** for all output structs. Every command supports `--output-format json`.
- **`Graph::load`** to load the full database (accepts only `&Config`, uses `config.resolved_db_root()?` internally; stores raw `FileScanResult`s in `graph.results` for commands needing heading-level data).
- **`config.resolved_db_root()?`** to get the resolved database path (panics if not set — main ensures it's set before dispatch).
- **Use `ctx.print_count`**, **`ctx.print_json`**, and **`ctx.print_ndjson`** from `OutputContext` for output dispatch. Every command receives `&OutputContext`.

### Heading nodes

Each org-mode heading with an `:ID:` property becomes a **first-class `Node`** in the graph.

## Command pattern

Every command's `run()` follows the same pattern:
1. Accept `&Config, &OutputContext, ...specific_args...`
2. Call `Graph::load(config)` if the full graph is needed
3. Perform the command logic
4. Dispatch output using `ctx.print_json()`, `ctx.print_ndjson()`, or the `OutputFormat` match

## Testing patterns

- **Unit tests** live in each module under `#[cfg(test)] mod tests { ... }`.
- **Integration tests** in `tests/integration/` spawn the actual binary with a temp mock DB (one file per subcommand).
- **Property-based tests** in `graph.rs` and `parser.rs` use `proptest` (random Graph::Build fuzzing, slug roundtrip, UUID format, panic fuzzing)
- **JSON output testing**: All commands are tested with `--output-format json` via `test_all_commands_json`, verifying valid JSON output for every command.
- Mock DB helper in `tests/integration/mod.rs::setup_db()` creates a 12+ note graph with duplicate UUIDs, broken links, filetags, aliases, and headings.

## Feature flags

| Feature | Default | Description                              |
|---------|---------|------------------------------------------|
| `embed` | off     | Enables `--embed` flag in `query` and `suggest` commands for embedding-based semantic similarity via `fastembed`. Build with `--features embed`. |

## Command pipelining

Commands can be chained via Unix pipes using NDJSON:

```
pkms query "topic" --output-format ndjson | pkms get --links --from-stdin
pkms resolve --tags "ai" --output-format ndjson | pkms get --links --from-stdin
pkms stats --hubs --output-format ndjson | pkms get --links --no-content --from-stdin
```

Consumer commands (`get`, `suggest`, `validate`, `context`) accept `--from-stdin`
to read UUIDs from stdin. They auto-detect piped stdin when no target is given.

NDJSON output emits one JSON object per line (each with a `uuid` field).

- Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`
- Consumers: `get`, `suggest`, `validate`, `context`

## Related

The `skill/pkms-manager/` directory contains an AI agent skill for efficiently using the pkms tool. See `SKILL.md` there for usage workflows.
