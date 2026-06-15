# AGENTS.md - pkms Project Guide for AI Agents

This file is the compact operating guide for agents working in this repository.
Keep detailed command recipes and contributor workflows in
[docs/development.md](docs/development.md).

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
- [Full CI gate](docs/development.md#full-ci-gate)
- [Adding or changing commands](docs/development.md#adding-or-changing-commands)
- [Output contracts](docs/development.md#output-contracts)
- [Task-system changes](docs/development.md#task-system-changes)
- [Diagnostics](docs/development.md#diagnostics)
- [Documentation and release workflow](docs/development.md#documentation-and-release-workflow)

All checks that are part of the selected workflow must pass. Fix clippy warnings
directly; do not suppress lints unless the user explicitly asks.

## Project Map

The filesystem structure is documented in
[docs/development.md#project-structure](docs/development.md#project-structure).
Prefer checking the current source over trusting any map when a file has moved
or behavior has changed.

## Architecture Invariants

`pkms` is primarily a stateless single-run CLI:

- Parse args, resolve config, load org files, compute, print, exit.
- Do not introduce persistent caches, databases, daemons, or watch mode.
- `pkms serve` is the explicit foreground local HTTP viewer exception. It must
  keep no persistent derived state.
- `Graph::load()` re-scans and re-parses `.org` files for each invocation.
- `Config::load()` reads optional user config; `Config::resolve()` produces a
  `ResolvedConfig`.
- `db_root` resolution happens once through CLI `--db`, then `PKMS_DB_ROOT`, then
  config file `db_root`.
- Commands read paths from `ResolvedConfig`, usually through
  `resolved_db_root()`, `resolve_new_notes_dir()`, or
  `resolve_ignore_patterns()`.

## Command and Output Conventions

- Define CLI args in `src/cli.rs` and dispatch from `src/runner.rs`.
- Put behavior in `src/commands/<name>.rs`, or `src/commands/<name>/` for a
  namespace with subcommands.
- Use option structs for command input when arguments are more than trivial.
- Command implementations usually accept `&ResolvedConfig` and `&OutputContext`;
  use `&CommandContext` when shared graph/workspace loader helpers are useful.
- Load the graph or workspace only when the command needs that data.
- Return `anyhow::Result`; `check` may return an `ExitCode` for unhealthy
  database state.
- Derive `serde::Serialize` for command output structs.
- Every command should support `--output-format json`; stream-like commands
  should support `ndjson` when practical.
- Dispatch structured output through `OutputContext` helpers:
  `print_json`, `print_ndjson`, or `print_json_adaptive`.
- For read-only commands with non-trivial shaping, prefer an internal
  `execute(...)` / `render(...)` split with pure text-rendering helpers. Do not
  force this onto side-effect-first commands such as `open`, `new`, `fix`, task
  mutations, or long-running `serve`.

## Core Domain Rules

Canonical task IDs:

- TODO headings receive deterministic global IDs shared by `task list`,
  `task agenda`, and ID-first task actions such as `task p<ID> show` and
  `task p<ID> open`.
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
- Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`.
- Consumers: `get`, `suggest`, `validate`, `context`, `task list`.

Feature flags:

| Feature | Default | Description |
|---------|---------|-------------|
| `todoist` | off | Enables Todoist task reads and writes through `ureq`. |
| `web` | off | Enables the local `serve` web viewer and static rendering through `katex` and `syntect`. |

## Scenario Guide

Bug reports: first reproduce with the binary built from the current checkout.
Use the workflow in [Bug reproduction](docs/development.md#bug-reproduction).
Do not rely on an installed `pkms` binary unless the user asks to debug it.

Normal code changes: keep the edit narrow, add focused tests near the changed
behavior, and use [Local development loop](docs/development.md#local-development-loop).

Before committing or handing off: run the
[Fast pre-commit gate](docs/development.md#fast-pre-commit-gate), plus any
[Feature-specific checks](docs/development.md#feature-specific-checks) for code
you touched. CI owns the full matrix in
[Full CI gate](docs/development.md#full-ci-gate).

Adding or changing commands: follow
[Adding or changing commands](docs/development.md#adding-or-changing-commands),
update integration tests, and update user docs for visible behavior.

Task behavior: preserve canonical task IDs and source semantics. Follow
[Task-system changes](docs/development.md#task-system-changes).

Structured output or pipelining: preserve JSON and NDJSON contracts. Follow
[Output contracts](docs/development.md#output-contracts) and update schemas under
`skills/pkms-manager/schemas/` when output changes.

Todoist changes: keep token handling out of logs and stdout. Use
`PKMS_LOG_HTTP=1` only for scrubbed request metadata, and run the Todoist checks
from [Feature-specific checks](docs/development.md#feature-specific-checks).

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
