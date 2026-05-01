# AGENTS.md — pkms project guide for AI agents

## Build & Test

```bash
cargo build                    # Build the binary
cargo test                     # Run all unit + integration tests (26 tests, ~1s)
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
tests/
  integration.rs    # 15 integration tests with temp mock DB
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

## Testing patterns

- **Unit tests** live in each module under `#[cfg(test)] mod tests { ... }`
- **Integration tests** in `tests/integration.rs` spawn the actual binary with a temp mock DB
- **Property-based tests** in `parser.rs` use `proptest` (slug roundtrip, UUID format, panic fuzzing)
- Mock DB helper in `tests/integration.rs::setup_db()` creates a 5-note graph with known broken links and orphans

## When modifying the database

The test DB at `~/Documents/org` contains ~762 real notes. Commands run against it with:
```bash
target/debug/pkms --db ~/Documents/org <command>
```

Changes to files in `~/Documents/org` are tracked by git. Always run `check` after modifications to verify no new issues introduced.

## Related

The `skill/pkms-manager/` directory contains an AI agent skill for efficiently using the pkms tool. See `SKILL.md` there for usage workflows.
