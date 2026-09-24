# AGENTS.md - pkms Project Guide for AI Agents

This file is the compact operating guide for agents working in this repository.
Start from [docs/index.md](docs/index.md) for the full documentation map. Keep
detailed command recipes, contributor workflows, and crate-specific architecture
in `docs/`.

## Project Target

`pkms` is a local-first personal knowledge management CLI for org-roam style
notes. The project optimizes for predictable command-line behavior, parseable
output, deterministic IDs, and safe operation on a user's notes database.

The philosophy is simple:

- Keep the tool stateless by default: read files, compute, print, exit.
- Prefer explicit CLI behavior over background services or hidden state.
- Keep human output useful and structured output stable.
- Optimize the fresh scan, parse, and graph build path instead of adding
  persistent derived state.
- Treat notes databases as user data: inspect examples carefully, avoid broad
  assumptions, and preserve parseable stdout.

## Workflow Source of Truth

Use [docs/development.md](docs/development.md) for specific workflows:

- [Bug reproduction](docs/development.md#bug-reproduction)
- [Local development loop](docs/development.md#local-development-loop)
- [Fast pre-commit gate](docs/development.md#fast-pre-commit-gate)
- [Feature-specific checks](docs/development.md#feature-specific-checks)
- [Adding or changing commands](docs/development.md#adding-or-changing-commands)
- [Output contracts](docs/development.md#output-contracts)
- [Task-system changes](docs/development.md#task-system-changes)
- [Diagnostics](docs/development.md#diagnostics)
- [Documentation and release workflow](docs/development.md#documentation-and-release-workflow)

All checks that are part of the selected workflow must pass. Fix clippy warnings
directly; do not suppress lints unless the user explicitly asks.

## Project Map

Use these docs to route work before opening source files:

- [Documentation Index](docs/index.md): full user, agent, scenario, and crate
  documentation map.
- [Architecture](docs/architecture.md): runtime shape, crate boundaries,
  command flow, data flows, and feature flags.
- [Project Structure](docs/development.md#project-structure): filesystem map
  and local development layout.
- [Scenario Guide](docs/scenarios.md): user and agent workflows for note
  database commands, task commands, web, RAG, and pipelines.

Crate-specific docs:

- [pkms](docs/crates/pkms.md): CLI parsing, config, dispatch, output, and
  command adapters.
- [pkms-org](docs/crates/pkms-org.md): org discovery, parsing, graph, snapshots,
  and edit primitives.
- [pkms-db](docs/crates/pkms-db.md): note database commands and link checks.
- [pkms-task](docs/crates/pkms-task.md): task IDs, filters, providers, and
  mutations.
- [pkms-rag](docs/crates/pkms-rag.md): retrieval index, embeddings, search,
  retrieval, and RAG HTTP API.
- [pkms-web](docs/crates/pkms-web.md): local web viewer, rendering, routes, and
  assets.
- [pkms-tokens](docs/crates/pkms-tokens.md): token encoding, counting, and
  truncation leaf utilities.

Prefer checking the current source over trusting any map when a file has moved
or behavior has changed.

## Architecture Invariants

`pkms` is primarily a stateless single-run CLI:

- Parse args, resolve config, load org files, compute, print, exit.
- Do not introduce implicit persistent caches, databases, daemons, or watch
  mode.
- `pkms serve` is the explicit foreground local HTTP viewer exception. It must
  keep no persistent derived state.
- `pkms rag` is the explicit derived-state exception: it owns a local SQLite
  retrieval index, and `pkms rag serve` is a foreground server rather than a
  daemon or watcher. Org files remain authoritative.
- `Graph::load_from()` and `OrgSnapshot::load()` re-scan and re-parse `.org`
  files for each invocation.
- `Config::load_from()` reads optional user config; `Config::resolve()`
  produces a `ResolvedConfig` from that config and captured `RuntimeInputs`.
- `db_root` resolution happens once through CLI `--db`, then `PKMS_DB_ROOT`, then
  config file `db_root`.
- Commands read paths from `ResolvedConfig`, usually through
  `resolved_db_root()`, `resolve_new_notes_dir()`, or
  `resolve_ignore_patterns()`.

## Command and Output Conventions

- Define CLI args in `crates/pkms/src/cli.rs` and dispatch from
  `crates/pkms/src/runner.rs`.
- Put umbrella command wiring/output in `crates/pkms/src/commands/<name>.rs`, or
  `crates/pkms/src/commands/<name>/` for a namespace with subcommands.
- Put reusable domain behavior in `pkms-org`, `pkms-db`, `pkms-task`,
  `pkms-rag`, or `pkms-web`; put token encoding/counting primitives in the
  leaf `pkms-tokens` crate according to the boundaries documented in
  [docs/architecture.md](docs/architecture.md) and
  [crate-specific docs](docs/index.md#crate-documentation).
- Use option structs for command input when arguments are more than trivial.
- Command implementations usually accept `&CommandContext` for shared resolved
  config and output access.
- Load a graph or one-scan org snapshot only when the command needs that data.
- Return `anyhow::Result`; `check` may return an `ExitCode` for unhealthy
  database state.
- Derive `serde::Serialize` for command output structs.
- Commands should support `--output-format json` unless they document a
  text-only contract, such as `pkms rag index`; stream-like commands should
  support `ndjson` when practical.
- Dispatch structured output through `OutputContext` helpers:
  `print_json`, `print_ndjson`, or `print_json_adaptive`.
- For read-only commands with non-trivial shaping, prefer an internal
  `execute(...)` / `render(...)` split with pure text-rendering helpers. Do not
  force this onto side-effect-first commands such as `open`, `new`, `fix`, task
  mutations, or long-running `serve`.

## Core Domain Rules

Canonical task IDs:

- TODO headings receive deterministic global IDs shared by `task list`,
  `task agenda`, and ID-first task actions such as `task <ID> show` and
  `task <ID> open`.
- IDs are based on task status grouping and stable ordering within the parsed
  database.
- Filtered views can show non-contiguous IDs because excluded tasks still occupy
  their global positions.
- Use the shared task-index helpers rather than implementing a parallel task ID
  scheme.

Heading nodes:

- Each org-mode heading with an `:ID:` property is a first-class graph node.
- UUID resolution, duplicate-ID validation, links, and neighborhoods must account
  for both note-level and heading-level IDs.

Command pipelining:

- NDJSON producers emit one JSON object per line, usually with a `uuid` field.
- Consumers read targets from stdin via automatic pipe detection or
  `--from-stdin`.
- Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`,
  `mentions`, and,
  with the `rag` feature, `rag search` and `rag retrieve`.
- Consumers: `get`, `suggest`, `validate`, `task list`.

Feature flags:

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | off | Enables the local `serve` web viewer and static rendering through `katex` and `syntect`. |
| `ssh` | off | Enables remote SSH `file:` link checks. |
| `rag` | off | Enables `pkms rag` local retrieval and the optional `pkms-rag` dependency. |

## Scenario Guide

Use [docs/scenarios.md](docs/scenarios.md) to choose the right command family.

Bug reports: first reproduce with the binary built from the current checkout.
Use the workflow in [Bug reproduction](docs/development.md#bug-reproduction).
Do not rely on an installed `pkms` binary unless the user asks to debug it.

Note database commands: use
[Note Database Commands](docs/note-database-commands.md) for scenarios and
[Command Reference](docs/commands.md) for flags.

Task commands: use [TODO and Agenda](docs/todo-agenda.md) for user workflows
and [Task System Design](docs/task-system.md) before changing task internals.

RAG commands: use [RAG Retrieval](docs/rag.md) for run instructions and
[pkms-rag](docs/crates/pkms-rag.md) for implementation boundaries.

Web viewer commands: use [Web Viewer](docs/web.md) for run instructions and
[pkms-web](docs/crates/pkms-web.md) for implementation boundaries.

Normal code changes: keep the edit narrow, add focused tests near the changed
behavior, and use [Local development loop](docs/development.md#local-development-loop).

Before committing or handing off: run the
[Fast pre-commit gate](docs/development.md#fast-pre-commit-gate). Use
[Feature-specific checks](docs/development.md#feature-specific-checks) for
focused local debugging when useful.

Adding or changing commands: follow
[Adding or changing commands](docs/development.md#adding-or-changing-commands),
update integration tests, and update user docs for visible behavior.

Task behavior: preserve canonical task IDs and source semantics. Follow
[Task-system changes](docs/development.md#task-system-changes).

Structured output or pipelining: preserve JSON and NDJSON contracts. Follow
[Output contracts](docs/development.md#output-contracts) and update schemas under
`skills/pkms-manager/schemas/` when output changes.

Web viewer changes: keep `serve` foreground-only and free of persistent derived
state. Run the web checks from
[Feature-specific checks](docs/development.md#feature-specific-checks).

Documentation changes: keep `README.md` concise, put detailed usage in `docs/`,
and update `skills/pkms-manager/` only when CLI behavior or agent workflows
change. See
[Documentation and release workflow](docs/development.md#documentation-and-release-workflow).

Commits and releases: only commit when the user asks. Before a release commit,
update `Cargo.toml` and the corresponding `Cargo.lock` package entry, then use
the release workflow in
[Documentation and release workflow](docs/development.md#documentation-and-release-workflow).
