# Crate: pkms

`crates/pkms` is the umbrella binary crate. It owns CLI parsing, configuration
resolution, command dispatch, output formatting, logging setup, and
cross-domain orchestration.

## Responsibilities

- Define the user-facing CLI in `src/cli.rs` and `src/cli/task.rs`.
- Build application state from CLI arguments and config in `src/app.rs` and
  `src/config/`.
- Capture environment variables and process paths once in immutable
  `RuntimeInputs`; resolve precedence through pure config functions.
- Dispatch commands from `src/runner.rs`.
- Provide shared resolved config and output context in `src/command_context.rs`.
- Render text, JSON, and NDJSON through `src/output.rs` and `src/output/`.
- Keep command adapter modules under `src/commands/` thin.
- Own editor process parsing, launching, and exit-status diagnostics.
- Map CLI arguments, environment variables, and `ResolvedConfig` into
  domain-specific config structs for `pkms-org`, `pkms-db`, `pkms-task`, and
  `pkms-web`, and, when the `rag` feature is enabled, wire resolved RAG options
  into `pkms-rag`.
- List direct note and heading tag assignments through `pkms tags`, resolve
  live note/task targets for `pkms tags suggest`, and apply additive source
  mutations while `pkms-rag` owns recommendation scoring.
- Parse shared discovery scope flags and map them to the `pkms-org` scope
  domain type after resolving `db_root`.

## Main Modules

| Module | Purpose |
|--------|---------|
| `src/main.rs` | Thin binary entry point. |
| `src/lib.rs` | Narrow `run()` application entry point; implementation modules are private. |
| `src/cli.rs` | Top-level `clap` parser and command enum. |
| `src/cli/task.rs` | Task namespace parser, filters, modifiers, and help text. |
| `src/runner.rs` | Command dispatch and exit-code conversion. |
| `src/command_context.rs` | Shared access to resolved config and output. |
| `src/app.rs` | Startup state construction and structured startup/command errors. |
| `src/config/` | Config parsing, defaults, path precedence, columns, and per-domain config mapping. |
| `src/commands/` | CLI adapters and output rendering for command namespaces. |
| `src/commands/scope.rs` | Maps shared CLI note-scope flags to the domain filter. |
| `src/output.rs` and `src/output/` | Output format selection, structured output, task tables, and terminal markup. |
| `src/input.rs` | Stdin target detection, target parsing, and command input helpers. |
| `src/logging.rs` | `PKMS_LOG`, `PKMS_LOG_FORMAT`, and stderr logging setup. |
| `src/environment.rs` | Immutable process inputs captured once at startup. |
| `src/editor.rs` | Typed editor command parsing and process execution. |

## Command Adapter Rules

- Define arguments in `cli.rs` or `cli/task.rs`, not inside domain crates.
- Resolve process environment at the `pkms` boundary and pass typed values into
  domain crates.
- Dispatch from `runner.rs`.
- Keep reusable command behavior in the domain crate when possible.
- Use option structs when a command has more than trivial input.
- Load a graph or one-scan org snapshot only when the command needs it.
- Dispatch structured output through `OutputContext` helpers.
- For read-only commands with non-trivial shaping, prefer an
  `execute(...)` / `render(...)` split.

## Boundaries

This crate may depend on all domain crates and on `pkms-tokens`, but domain and
leaf crates must not depend on `pkms`. The library facade intentionally exposes
only `run()`; CLI implementation modules remain private. If a change tempts you
to import `pkms` from another crate, move shared behavior into the owning domain
crate or keep it in the command adapter.

## Related Docs

- [Architecture](../architecture.md)
- [Development](../development.md)
- [Command Reference](../commands.md)
- [JSON and NDJSON Output](../json-output.md)
- [Pipelining](../pipelining.md)
