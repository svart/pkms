# pkms

`pkms` is a CLI tool for navigating, managing, and validating
[org-roam](https://www.orgroam.com/) personal knowledge bases.

It reads org files from disk, builds an in-memory graph, performs one command,
prints output, and exits. It does not keep a cache, database, daemon, watch
process, or server. Performance work should preserve this stateless single-run
model and optimize the fresh read/parse path.

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
pkms todo --columns Id,Date,Prio,Note,Heading
pkms agenda --today
pkms show 5
pkms open 5
pkms new "My Note" --create --tags "topic,project"
```

Commands support text output by default and structured output with
`--output-format json` or `--output-format ndjson`.

```bash
pkms query "rust" --output-format ndjson | pkms get --links --from-stdin
```

## Main Concepts

- Notes are org-roam `.org` files with UUID `:ID:` properties and `#+title:`.
- Filetags use the canonical `#+filetags: :tag1:tag2:` form.
- Internal links use `[[id:<uuid>][description]]`.
- Headings with `:ID:` properties are first-class graph nodes.
- TODO headings get deterministic global IDs shared by `todo`, `agenda`,
  `show`, and `open`.
- `query` and `suggest` support embedding mode only when built with
  `--features embed`.

## Documentation

- [Installation](docs/installation.md)
- [Configuration](docs/configuration.md)
- [Notes Database Format](docs/database-format.md)
- [Command Reference](docs/commands.md)
- [TODO and Agenda](docs/todo-agenda.md)
- [Pipelining](docs/pipelining.md)
- [JSON and NDJSON Output](docs/json-output.md)
- [Maintenance Workflows](docs/workflows.md)
- [Development](docs/development.md)

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success / healthy |
| 1 | Issues found / error |
