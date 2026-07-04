# Crate: pkms

`crates/pkms` is the umbrella binary crate. It owns CLI parsing, configuration
resolution, command dispatch, output formatting, logging setup, and
cross-domain orchestration.

## Responsibilities

- Define the user-facing CLI in `src/cli.rs` and `src/cli/task.rs`.
- Build application state from CLI arguments and config in `src/app.rs` and
  `src/config.rs`.
- Dispatch commands from `src/runner.rs`.
- Provide shared loaders and context in `src/command_context.rs`.
- Render text, JSON, and NDJSON through `src/output.rs` and `src/output/`.
- Keep command adapter modules under `src/commands/` thin.
- Map `ResolvedConfig` into domain-specific config structs for `pkms-org`,
  `pkms-db`, `pkms-task`, and `pkms-web`, and wire RAG CLI/env options into
  `pkms-rag`.

## Main Modules

| Module | Purpose |
|--------|---------|
| `src/main.rs` | Thin binary entry point. |
| `src/lib.rs` | Library surface used by tests. |
| `src/cli.rs` | Top-level `clap` parser and command enum. |
| `src/cli/task.rs` | Task namespace parser, filters, modifiers, and help text. |
| `src/runner.rs` | Command dispatch and exit-code conversion. |
| `src/command_context.rs` | Shared access to resolved config, output, graph, and workspace loaders. |
| `src/config.rs` | Config file parsing, `db_root` resolution, and per-domain config mapping. |
| `src/commands/` | CLI adapters and output rendering for command namespaces. |
| `src/output.rs` | Output format selection and structured output helpers. |
| `src/input.rs` | Stdin target detection, target parsing, and command input helpers. |
| `src/logging.rs` | `PKMS_LOG`, `PKMS_LOG_FORMAT`, and stderr logging setup. |

## Command Adapter Rules

- Define arguments in `cli.rs` or `cli/task.rs`, not inside domain crates.
- Dispatch from `runner.rs`.
- Keep reusable command behavior in the domain crate when possible.
- Use option structs when a command has more than trivial input.
- Load graph or workspace data only when the command needs it.
- Dispatch structured output through `OutputContext` helpers.
- For read-only commands with non-trivial shaping, prefer an
  `execute(...)` / `render(...)` split.

## Boundaries

This crate may depend on all domain crates, but domain crates must not depend on
`pkms`. If a change tempts you to import `pkms` from a domain crate, move the
shared behavior down into the correct domain crate or keep it in the adapter.

## Related Docs

- [Architecture](../architecture.md)
- [Development](../development.md)
- [Command Reference](../commands.md)
- [JSON and NDJSON Output](../json-output.md)
- [Pipelining](../pipelining.md)
