# AGENTS.md — pkms project guide for AI agents

## Build & Test

```bash
cargo build                    # Build the binary
cargo test                     # Run all unit + integration tests (124+ tests, ~1s)
cargo test --test integration  # Integration tests only (mock DB)
target/debug/pkms --help       # Verify CLI works
```

No lint or formatting commands are configured. The project has no clippy or rustfmt CI.

## Project structure

```
src/
  main.rs           # CLI dispatch — match on Command enum, call commands::*::run()
  cli.rs            # clap derive structs — Cli, Command enum (all subcommands)
  config.rs         # ~/.config/pkms.toml loading, merging with CLI --db flag
  discovery.rs      # Recursive .org file discovery with ignore patterns
  parser.rs         # org-mode parser: IDs, titles, filetags, aliases, refs, links, headings
  graph.rs          # In-memory graph: Node, Link, Graph, BFS pathfinding, subgraph
  commands/         # One file per subcommand
    mod.rs          # Module declarations only
    info.rs         # Show resolved config
    check.rs        # Full DB health scan, returns healthy: bool
    validate.rs     # Single note health check
    stats.rs        # Comprehensive statistics
    orphans.rs      # List orphan notes
    broken.rs       # List broken links
    hubs.rs         # List most-connected notes
    resolve.rs      # Fast UUID resolution (header-only scan, ~0.5s)
    fix.rs          # Replace broken UUIDs across all files
    suggest.rs      # Find related notes by multi-factor scoring
    get.rs          # Retrieve note with neighbors at depth N
    path.rs         # Shortest path (BFS) between two notes
    subgraph.rs     # Export subgraph with stats
    query.rs        # Fuzzy search titles + content
    tags.rs         # List filetags with counts
    new.rs          # Generate UUID + filename for new note
    context.rs      # Build AI context window with token budget
schemas/            # JSON Schema files for every command's --json output
tests/
  integration.rs    # 79 integration tests with temp mock DB
```

## How to add a new command

1. **`src/cli.rs`** — Add variant to `Command` enum with clap attributes
2. **`src/commands/<name>.rs`** — Create file with `pub fn run(...)` that accepts `&Config, json: bool, verbose: bool, ...` and returns `anyhow::Result<()>`
3. **`src/commands/mod.rs`** — Add `pub mod <name>;`
4. **`src/main.rs`** — Add `Command::<Name> => commands::<name>::run(...)` arm
5. **`tests/integration.rs`** — Add test calling the binary

## Code conventions

- **No comments** unless the logic is non-obvious. Code should be self-documenting.
- **`anyhow::Result`** for all fallible functions. No custom error types.
- **`serde::Serialize`** for all output structs. Every command supports `--json`.
- **`Graph::load(config, db_cli, verbose)`** to load the full database (discovers + parses 750+ files, ~3s). Expensive — cache results when possible.
- **`resolve`** command is fast (~0.5s) because it scans only file headers. Use for quick lookups.
- **`#[allow(dead_code)]`** on struct fields kept for future use. Remove if never needed after implementation.
- **Prefix unused params** with `_` (e.g., `_verbose`).

## Graph data model

```
Node { uuid, title, path, filetags, aliases, refs, content_hash, outgoing: Vec<Link>, headings_count }
Link::Internal(String) | File(String) | Url(String) | Attachment(String)
Graph { nodes: HashMap<uuid, Node>, path_to_uuid, title_to_uuid, backlinks, broken_links, ... }
```

`Graph::build(results)` processes `FileScanResult`s and produces the graph with backlinks and broken link detection. The `load()` static method is the main entry point — it discovers files, parses them, and builds the graph.

## Command pattern

Every command's `run()` follows the same pattern:
1. Accept `&Config, json: bool, verbose: bool, ...specific_args..., db_cli: Option<&Path>`
2. Call `Graph::load(config, db_cli, verbose)` if the full graph is needed
3. Perform the command logic
4. If `json`, print `serde_json::to_string_pretty(&output_struct)?`
5. Otherwise, print human-readable output with `println!`

## JSON output schemas

Every command's `--json` output has a corresponding JSON Schema in `schemas/<command>.json`.
These schemas define the exact structure and types for reliable programmatic consumption.

| Command     | Schema file             | Top-level keys |
|-------------|------------------------|----------------|
| `check`     | `schemas/check.json`   | `db_root`, `stats`, `duplicates`, `broken_links`, `failed_files`, `healthy` |
| `stats`     | `schemas/stats.json`   | `db_root`, `total_notes`, `total_links`, `hubs`, `directories`, `recent_notes` |
| `resolve`   | `schemas/resolve.json` | `query`, `total`, `results[]` (uuid, title, path, filetags, aliases) |
| `suggest`   | `schemas/suggest.json` | `target`, `target_uuid`, `suggestions[]` (uuid, title, score, scores{}, reasons[]) |
| `query`     | `schemas/query.json`   | `query`, `total_results`, `results[]` (uuid, title, score, matches[], content_matches[]) |
| `context`   | `schemas/context.json` | `target`, `context`, `estimated_tokens`, `depth` |
| `validate`  | —                      | `uuid`, `title`, `path`, `healthy`, `issues[]`, `broken_internal[]` |
| `orphans`   | —                      | `count`, `orphans[]` (uuid, title, path, filetags) |
| `broken`    | —                      | `count`, `links[]` (source_uuid, source_title, target_uuid) |
| `hubs`      | —                      | `limit`, `hubs[]` (rank, uuid, title, degree, outgoing, incoming) |
| `tags`      | —                      | `tags[]` (tag, count, notes[]) |
| `get`       | —                      | `node` (uuid, title, path, filetags), `neighbors` |
| `path`      | —                      | `from`, `to`, `found`, `hops`, `path[]` (uuid, title) |
| `subgraph`  | —                      | `root_uuid`, `root_title`, `vertex_count`, `edge_count`, `nodes[]`, `edges[]` |
| `fix`       | —                      | `broken_uuid`, `replacement_uuid`, `replacement_title`, `files_affected[]`, `total_replacements`, `applied` |
| `new`       | —                      | `uuid`, `filename`, `path`, `title`, `created` |
| `info`      | —                      | `config`, `config_path`, `cli_overrides` |

## Testing patterns

- **Unit tests** live in each module under `#[cfg(test)] mod tests { ... }`
- **Integration tests** in `tests/integration.rs` spawn the actual binary with a temp mock DB
- **Property-based tests** in `graph.rs` and `parser.rs` use `proptest` (random Graph::Build fuzzing, slug roundtrip, UUID format, panic fuzzing)
- **JSON schema validation**: All commands are tested with `--json` via `test_all_commands_json`, verifying valid JSON output for every command
- Mock DB helper in `tests/integration.rs::setup_db()` creates a 10+ note graph with duplicate UUIDs, broken links, filetags, aliases, and headings

## When modifying the database

The test DB at `~/Documents/org` contains ~762 real notes. Commands run against it with:
```bash
target/debug/pkms --db ~/Documents/org <command>
```

Changes to files in `~/Documents/org` are tracked by git. Always run `check` after modifications to verify no new issues introduced.

## CLI automation flags

| Flag              | Description                                      |
|-------------------|--------------------------------------------------|
| `--no-header`     | Suppress column headers in human output          |
| `--count`         | Show only the result count                       |
| `--from-stdin`    | Read targets from stdin (one per line)           |
| `--from-file`     | Read targets from a file (one per line)          |
| `--input-json`    | Read command parameters from a JSON file         |
| `--example`       | Show a usage example for the command and exit    |

## Related

The `skill/pkms-manager/` directory contains an AI agent skill for efficiently using the pkms tool. See `SKILL.md` there for usage workflows.
