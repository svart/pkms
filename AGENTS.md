# AGENTS.md — pkms project guide for AI agents

## Build, Lint & Test

After changes, run these commands **in this strict order**:

```bash
cargo fmt --check              # 1. Check formatting (fail if unformatted)
cargo clippy -- -D warnings    # 2. Lint with clippy (deny all warnings)
cargo build                    # 3. Build the binary
cargo test                     # 4. Run all unit + integration tests (112+ tests, ~1s)
cargo test --test integration  # 5. Integration tests only (mock DB)
target/debug/pkms --help       # 6. Verify CLI works
```

IMPORTANT: All tests **MUST** pass. Fix all issues which appear when running these commands.
IMPORTANT: Never disable clippy lints by yourself. Always fix issues, mentioned by clippy.

## Committing changes

When you are done with changes, before committing the work into git do next:
- check that AGENTS.md, README.md and SKILL.md has necessary information, if not update them accordingly;
- update the version of the package in Cargo.toml.

Then commit changes.

## Project structure

```
src/
  main.rs           # CLI dispatch — match on Command enum, call commands::*::run()
  cli.rs            # clap derive structs — Cli, Command enum (all subcommands)
  config.rs         # ~/.config/pkms.toml loading, merging with CLI --db flag
  discovery.rs      # Recursive .org file discovery with ignore patterns
  parser.rs         # org-mode parser: IDs, titles, filetags, aliases, refs, links, headings
  util.rs           # Shared helpers: short_uuid, path_string
  output.rs         # OutputContext: format dispatch (Text/Json/Ndjson), print helpers
  graph/            # In-memory graph module (split into submodules)
    mod.rs          # Struct defs: Node, Graph, FileScanResult; load/scan/find_node/resolve_target
    builder.rs      # Graph::build constructor
    traversal.rs    # get_neighbors, find_shortest_path (BFS)
    search.rs       # search, search_content, all_tags
    analytics.rs    # hubs, orphan_nodes, broken_links_list, stats, directory_breakdown
    tests.rs        # Unit + proptest tests for graph
  commands/         # One file per subcommand
    mod.rs          # Module declarations only
    info.rs         # Show resolved config
    check.rs        # Full DB health scan, returns healthy: bool
    validate.rs     # Single note health check
    stats.rs        # Comprehensive statistics (+ --hubs, --tags flags)
    orphans.rs      # List orphan notes
    resolve.rs      # UUID resolution
    fix.rs          # Replace broken UUIDs across all files
    suggest.rs      # Find related notes by multi-factor scoring (takes UUID only)
    get.rs          # Retrieve note with neighbors at depth N
    path.rs         # Shortest path (BFS) between two notes
    query.rs        # Fuzzy search titles + content
    new.rs          # Generate UUID + filename for new note
    context.rs      # Build AI context window with token budget
tests/
  integration.rs    # 65 integration tests with temp mock DB
```

## How to add a new command

1. **`src/cli.rs`** — Add variant to `Command` enum with clap attributes
2. **`src/commands/<name>.rs`** — Create file with `pub fn run(...)` that accepts `&Config, &OutputContext, ...` and returns `anyhow::Result<()>`
3. **`src/commands/mod.rs`** — Add `pub mod <name>;`
4. **`src/main.rs`** — Add `Command::<Name> => commands::<name>::run(...)` arm
5. **`tests/integration.rs`** — Add test calling the binary

## Architecture: stateless single-run model

`pkms` is designed as a pure CLI pipeline — parse args, load org files from disk, compute, print, exit.
There is **no state persisted between invocations**:
- No cache files, databases, or daemon processes
- No server mode, watch mode, or background workers
- `Graph::load()` re-scans and re-parses every `.org` file on each invocation
- Every command's `run()` is a self-contained function with no side effects beyond reading/writing org files

**Do not introduce** statefulness (caches, databases, daemon mode) in future development.
If performance optimization is needed, compute from scratch on every run — do not persist state.

## Code conventions

- **No comments** unless the logic is non-obvious. Code should be self-documenting.
- **`anyhow::Result`** for all fallible functions. No custom error types.
- **`serde::Serialize`** for all output structs. Every command supports `--output-format json`.
- **`Graph::load(config, db_cli)`** to load the full database.
- **`resolve`** command scans only file headers.
- **`#[allow(dead_code)]`** on struct fields kept for future use. Remove if never needed after implementation.
- **Use `ctx.print_count`**, **`ctx.print_json`**, and **`ctx.print_ndjson`** from `OutputContext` for output dispatch. Every command receives `&OutputContext`.

## Graph data model

```
Node { uuid, title, path, filetags, aliases, refs, outgoing: Vec<Link>, headings_count }
Link::Internal(String) | File(String) | Url(String) | Attachment(String)
Graph { nodes: HashMap<uuid, Node>, path_to_uuid, title_to_uuid, backlinks, broken_links, ... }
```

`Graph::build(results)` processes `FileScanResult`s and produces the graph with backlinks and broken link detection. The `load()` static method is the main entry point — it calls `scan()` to discover and parse files, then `build()` to construct the graph. The `scan()` method can be called independently for commands that need the raw results.

## Command pattern

Every command's `run()` follows the same pattern:
1. Accept `&Config, &OutputContext, ...specific_args..., db_cli: Option<&Path>`
2. Call `Graph::load(config, db_cli)` if the full graph is needed
3. Perform the command logic
4. Dispatch output using `ctx.print_json()`, `ctx.print_ndjson()`, or the `OutputFormat` match
5. For text output, format with `println!`

## JSON output shapes

Every command's JSON (`--output-format json|ndjson`) output has a specific structure:

| Command     | Top-level keys |
|-------------|----------------|
| `check`     | `db_root`, `stats`, `duplicates`, `broken_links`, `broken_file_links`, `broken_attachment_links`, `failed_files`, `healthy` |
| `stats`     | `db_root`, `total_notes`, `total_links`, `internal_links`, `file_links`, `url_links`, `avg_links_per_note`, `orphans`, `broken_links`, `disk_size_bytes`, `directories[]`, `recent_notes[]` |
| `resolve`   | `query`, `total`, `results[]` (uuid, title, path, filetags, aliases) |
| `suggest`   | `target`, `target_uuid`, `suggestions[]` (uuid, title, score, scores{}, reasons[]) |
| `query`     | `query`, `total_results`, `results[]` (uuid, title, score, matches[], content_matches[]) |
| `context`   | `target`, `context`, `estimated_tokens`, `depth` |
| `validate`  | `uuid`, `title`, `path`, `filetags`, `aliases`, `headings`, `outgoing`, `incoming`, `outgoing_internal`, `broken_internal[]`, `broken_files[]`, `backlinks[]` (uuid, title), `issues[]`, `healthy` |
| `orphans`   | `count`, `orphans[]` (uuid, title, path, filetags) |
| `get`       | `node` (uuid, title, path, filetags), `neighbors` |
| `path`      | `from`, `to`, `found`, `hops`, `path[]` (uuid, title) |
| `fix`       | `broken_uuid`, `replacement_uuid`, `replacement_title`, `files_affected[]`, `total_replacements`, `applied` |
| `new`       | `uuid`, `filename`, `path`, `title`, `created` |
| `info`      | `config`, `config_path`, `cli_overrides` |

## Testing patterns

- **Unit tests** live in each module under `#[cfg(test)] mod tests { ... }`
- **Integration tests** in `tests/integration.rs` spawn the actual binary with a temp mock DB
- **Property-based tests** in `graph.rs` and `parser.rs` use `proptest` (random Graph::Build fuzzing, slug roundtrip, UUID format, panic fuzzing)
- **JSON output testing**: All commands are tested with `--output-format json` via `test_all_commands_json`, verifying valid JSON output for every command
- Mock DB helper in `tests/integration.rs::setup_db()` creates a 10+ note graph with duplicate UUIDs, broken links, filetags, aliases, and headings

## CLI flags

| Flag              | Description                                      |
|-------------------|--------------------------------------------------|
| `--output-format FMT` | Output format: `text`, `json`, or `ndjson`   |

## Related

The `skill/pkms-manager/` directory contains an AI agent skill for efficiently using the pkms tool. See `SKILL.md` there for usage workflows.
