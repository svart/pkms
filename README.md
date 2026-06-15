# pkms

`pkms` is a CLI tool for navigating, managing, and validating
[org-roam](https://www.orgroam.com/) personal knowledge bases.

It reads org files from disk, builds an in-memory graph, performs one command,
prints output, and exits. It does not keep a cache, database, daemon, or watch
process. When built with the `web` feature, the `serve` command is an explicit
foreground local web viewer; it still builds from the current files and keeps no
persistent derived state.

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

## Documentation

- [Installation](docs/installation.md)
- [Configuration](docs/configuration.md)
- [Notes Database Format](docs/database-format.md)
- [Command Reference](docs/commands.md)
- [TODO and Agenda](docs/todo-agenda.md)
- [Task System Design](docs/task-system.md)
- [Pipelining](docs/pipelining.md)
- [JSON and NDJSON Output](docs/json-output.md)
- [Maintenance Workflows](docs/workflows.md)
- [Development](docs/development.md)

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success / healthy |
| 1 | Issues found / error |
