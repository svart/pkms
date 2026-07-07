# pkms

`pkms` is a CLI tool for navigating, managing, and validating
[org-roam](https://www.orgroam.com/) personal knowledge bases.

It reads org files from disk, builds an in-memory graph, performs one command,
prints output, and exits. It does not keep a cache, database, daemon, or watch
process. When built with the `web` feature, the `serve` command is an explicit
foreground local web viewer; when built with the `rag` feature, `pkms rag`
manages an explicit local retrieval index. Both still build from current files
and keep no hidden persistent state.

## Quick Start

Install from a local checkout:

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run -- --db ~/Documents/org info
```

Create a persistent config:

```bash
pkms init-config --db ~/Documents/org
pkms info
```

Minimal `~/.config/pkms.toml`:

```toml
db_root = "/home/user/Documents/org"
new_notes_dir = "roam"
daily_notes_dir = "roam/daily"
ignore_patterns = [".attach", "*.bak"]

[tasks]
inbox = "Inbox"

[agenda]
open_todo_states = ["TODO", "WAITING", "IN-PROGRESS"]
closed_todo_states = ["DONE"]
```

`--db PATH` overrides the configured `db_root`. `PKMS_DB_ROOT` can also provide
the database root for a session.

## Core Examples

```bash
pkms info
pkms resolve --title "graph"
pkms query "distributed systems" --limit 5
pkms get <uuid-or-title> --links
pkms check
pkms task list --columns Id,Date,Prio,Note,Heading
pkms task agenda today
pkms task p5 show
pkms task p5 open
pkms new "My Note" --create --tags "topic,project"
pkms extract <heading-uuid> "New Note Title" --apply
```

Commands support text output by default and structured output with
`--output-format json` or `--output-format ndjson`.

The local web viewer is available in builds made with `--features web`:

```bash
pkms serve <target>
```

Local retrieval is available in builds made with `--features rag`:

```bash
pkms rag index
pkms rag retrieve "agenda inspect tasks" --limit 5
pkms rag serve --host 127.0.0.1 --port 7337
```

Set `[rag].rag_db` in `~/.config/pkms.toml` when the RAG SQLite index should
live somewhere other than the default `.data/pkms-rag.sqlite3`.
Set `[rag].fastembed_model_dir` when FastEmbed should load model files from a
local directory instead of downloading them.

```bash
pkms query "rust" --output-format ndjson | pkms get --links --from-stdin
```

## Main Concepts

- Notes are org-roam `.org` files with UUID `:ID:` properties and `#+title:`.
- Filetags use the canonical `#+filetags: :tag1:tag2:` form.
- Internal links use `[[id:<uuid>][description]]`.
- Headings with `:ID:` properties are first-class graph nodes.
- TODO headings get deterministic global IDs shared by `task list`,
  `task agenda`, and task ID actions such as `task p<ID> show`.
- `serve` is available only when built with `--features web`.
- `pkms rag` is available only when built with `--features rag`; it stores its
  retrieval index in SQLite and serves a foreground local HTTP API/UI with
  `pkms rag serve`.

## Architecture

The workspace is split into focused crates:

- `pkms-org`: org discovery, parsing, graph construction, workspace loading,
  typed task insertion, daily note creation, and raw org edit primitives.
- `pkms-db`: note database commands and link checks.
- `pkms-task`: task IDs, filtering, providers, mutations, typed org edit
  requests, and Todoist execution.
- `pkms-rag`: local retrieval indexing, embeddings, search, and HTTP API.
- `pkms-web`: the local `serve` HTTP viewer, HTML rendering, and static assets.
- `pkms`: the umbrella binary crate for CLI parsing, config mapping, dispatch,
  and output formatting.

See [Architecture](docs/architecture.md) for the command flow, dependency
boundaries, data flows, feature flags, and per-crate documentation.

## Documentation

- [Documentation Index](docs/index.md)
- [Installation](docs/installation.md)
- [Configuration](docs/configuration.md)
- [Notes Database Format](docs/database-format.md)
- [Command Reference](docs/commands.md)
- [Scenario Guide](docs/scenarios.md)
- [Note Database Commands](docs/note-database-commands.md)
- [TODO and Agenda](docs/todo-agenda.md)
- [Task System Design](docs/task-system.md)
- [Web Viewer](docs/web.md)
- [Pipelining](docs/pipelining.md)
- [JSON and NDJSON Output](docs/json-output.md)
- [RAG Retrieval](docs/rag.md)
- [Architecture](docs/architecture.md)
- [Maintenance Workflows](docs/workflows.md)
- [Development](docs/development.md)
