# Development

`pkms` is a stateless single-run CLI. Each invocation parses args, resolves the
database root once, reads org files from disk, computes the result, prints, and
exits.

Do not introduce persistent caches, daemon processes, server mode, or watch
mode. If performance needs improvement, optimize fresh discovery, parsing, and
graph construction.

## Build, Lint, and Test

Run these commands after changes:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo build --features=embed
cargo test
cargo test --test integration
target/debug/pkms --help
```

## Project Structure

```text
src/
  main.rs
  cli.rs
  config.rs
  discovery.rs
  org_date.rs
  parser.rs
  util.rs
  output.rs
  graph/
  commands/
tests/
  integration/
```

## Add a Command

1. Add a variant to `Command` in `src/cli.rs`.
2. Create `src/commands/<name>.rs`.
3. Export it from `src/commands/mod.rs`.
4. Dispatch it from `src/main.rs`.
5. Add integration tests under `tests/integration/`.
6. Update user docs and `skills/pkms-manager/` references when behavior changes.

## Output Conventions

All user-facing command output should support `--output-format json`. Streamable
commands should support `ndjson` where practical. Output structs should derive
`serde::Serialize`.
