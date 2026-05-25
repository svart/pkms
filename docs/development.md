# Development

`pkms` is a stateless single-run CLI. Each invocation parses args, resolves the
database root once, reads org files from disk, computes the result, prints, and
exits.

Do not introduce persistent caches, daemon processes, server mode, or watch
mode. If performance needs improvement, optimize fresh discovery, parsing, and
graph construction.

## Iterative Checks

During feature development, run the smallest checks that cover the code you just
changed. Keep the loop fast enough that failures stay close to the edit that
caused them.

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo test
```

For narrow changes, prefer focused tests before the broader suite:

```bash
cargo test <test-name>
cargo test --test integration <test-name>
```

When a change affects optional Todoist code, include the Todoist feature in the
focused loop:

```bash
cargo clippy --features todoist -- -D warnings
cargo test --features todoist <test-name>
cargo test --features todoist --test integration <test-name>
```

## Final Verification

Before committing release-ready work or after a broad behavior change, run the
full verification gate:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo clippy --features todoist -- -D warnings
cargo build
cargo build --features=embed
cargo build --features=todoist
cargo build --features=embed,todoist
cargo test
cargo test --features todoist
cargo test --test integration
cargo test --features todoist --test integration
cargo run -- --help
cargo run --features todoist -- --help
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
