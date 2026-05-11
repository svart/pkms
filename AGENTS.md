# AGENTS.md — pkms project guide for AI agents

## Build, Lint & Test

After changes, run these commands **in this strict order**:

```bash
cargo fmt --check              # 1. Check formatting (fail if unformatted)
cargo clippy -- -D warnings    # 2. Lint with clippy (deny all warnings)
cargo build                    # 3. Build the binary
cargo test                     # 4. Run all unit + integration tests (237+ tests, ~1s)
cargo test --test integration  # 5. Integration tests only (mock DB)
target/debug/pkms --help       # 6. Verify CLI works
```

IMPORTANT: All tests **MUST** pass. Fix all issues which appear when running these commands.
IMPORTANT: Never suppress clippy lints by yourself. Always fix issues, mentioned by clippy.

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
  org_date.rs       # Org-mode timestamp parser (SCHEDULED/DEADLINE dates)
  parser.rs         # org-mode parser: IDs, titles, filetags, aliases, refs, links, headings, priorities, SCHEDULED/DEADLINE
  util.rs           # Shared helpers: path_string, is_stdin_piped, read_stdin_ndjson
  output.rs         # OutputContext: format dispatch (Text/Json/Ndjson), print helpers
  graph/            # In-memory graph module (split into submodules)
    mod.rs          # Struct defs: Node, Graph, FileScanResult, SelfLinkEntry; load/scan/find_node/resolve_target, detect_self_links
    builder.rs      # Graph::build constructor
    traversal.rs    # get_neighbors, find_shortest_path (BFS)
    search.rs       # search, search_content, all_tags
    analytics.rs    # hubs, orphan_nodes, broken_links_list, stats, directory_breakdown
    tests.rs        # Unit + proptest tests for graph
  commands/         # One file per subcommand
    mod.rs          # Module declarations only
    agenda.rs       # Display upcoming and overdue items with SCHEDULED/DEADLINE dates
    todo.rs         # Display TODO items grouped by state
    info.rs         # Show resolved config
    check.rs        # Full DB health scan, returns healthy: bool
    validate.rs     # Single note health check
    stats.rs        # Comprehensive statistics (+ --hubs, --tags, --todos flags)
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
  integration/
    mod.rs          # Common test helpers (setup_db, run, run_json, etc.)
    check.rs        # Check command tests
    validate.rs     # Validate command tests
    stats.rs        # Stats command tests
    orphans.rs      # Orphans command tests
    query.rs        # Query command tests
    resolve.rs      # Resolve command tests
    suggest.rs      # Suggest command tests
    get.rs          # Get command tests
    path.rs         # Path command tests
    context.rs      # Context command tests
    info.rs         # Info command tests
    new.rs          # New command tests
    fix.rs          # Fix command tests
    agenda.rs       # Agenda command tests
    todo.rs         # Todo command tests
    error.rs        # Error path tests
    pipe.rs         # NDJSON pipeline tests
    snapshot.rs     # Snapshot tests
    all_commands.rs # Parametric JSON output test
```

## How to add a new command

1. **`src/cli.rs`** — Add variant to `Command` enum with clap attributes
2. **`src/commands/<name>.rs`** — Create file with `pub fn run(...)` that accepts `&Config, &OutputContext, ...` and returns `anyhow::Result<()>`
3. **`src/commands/mod.rs`** — Add `pub mod <name>;`
4. **`src/main.rs`** — Add `Command::<Name> => commands::<name>::run(...)` arm
5. **`tests/integration/<name>.rs`** — Create test file with `use super::*;` and `#[test]` functions

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
- **Use `ctx.print_count`**, **`ctx.print_json`**, and **`ctx.print_ndjson`** from `OutputContext` for output dispatch. Every command receives `&OutputContext`.

## Graph data model

```
Node { uuid, title, path, filetags, aliases, refs, outgoing: Vec<Link>, headings_count, heading_uuids, has_todos }
Link::Internal(String) | File(String) | Url(String) | Attachment(String)
Graph { nodes: HashMap<uuid, Node>, path_to_uuid, title_to_uuid, backlinks, broken_links, heading_uuid_to_primary, ... }
```

### Heading nodes

Each org-mode heading with an `:ID:` property becomes a **first-class `Node`** in the graph:
- Its own `uuid` (the heading's `:ID:`)
- Its own `title` (the heading text)
- Its own `outgoing` (links found under this heading subtree only)
- An explicit parent→child edge (`Link::Internal`) both ways between heading and parent
- File-level properties (`filetags`, `categories`, `aliases`, `refs`) inherited from the parent + heading-level tags
- `headings_count` / `heading_uuids` for sub-headings (non-zero only for headings with child heading UUIDs)

The tree structure is:
```
File Primary UUID   ← primary node (file-level links + parent→child edges to top-level headings)
├── Heading (no UUID)   ← not a graph node
├── Heading UUID A      ← graph node, parent=Primary
│   ├── Heading (no UUID)
│   └── Heading UUID B  ← graph node, parent=UUID A
├── Heading (no UUID)
└── Heading UUID C      ← graph node, parent=Primary
```

`Graph::heading_uuid_to_primary` maps each heading UUID to the file-level primary UUID.

### Link attribution

The parser (`parse_note`) attributes links to the active heading stack:
- Links before any heading → file-level (`parsed.outgoing`)
- Links under a heading → that heading's `heading.outgoing`
- Nested headings attribute correctly based on heading level
- Properties drawer links remain file-level

### Overlinking detection

`detect_overlinks` simply counts internal links per node (no dedup needed — each heading node has its own `outgoing`). A parent→child edge counts as one internal link, so heading nodes with only a parent link do not trigger overlinking alerts.

### Self-link detection

`detect_self_links` checks all nodes per file path. An `id:` link is only a self-link if `uuid == node.uuid` (prevents parent-child edges from being flagged).

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
| `check`     | `db_root`, `stats?`, `duplicates?`, `broken_links?`, `broken_file_links?`, `broken_attachment_links?`, `failed_files?`, `filetags_issues?` (path, title, issue), `agenda_issues?` (path, title, uuid, todo_count, issue — only for planned TODOs with SCHEDULED/DEADLINE), `self_links?` (source_uuid, source_title, link_type, target, suggestion?), `overlinks?` (source_uuid, source_title, target_uuid, target_title, count), `cross_links?` (source_uuid, source_title, target_uuid, target_title, source_to_target, target_to_source), `healthy` |
| `stats`     | `db_root`, `total_notes`, `total_links`, `internal_links`, `file_links`, `url_links`, `avg_links_per_note`, `orphans`, `broken_links`, `disk_size_bytes`, `directories[]`, `recent_notes[]` |
| `stats --todos` | `total_todo_headings`, `files_with_todos`, `by_state[]` (state, count) |
| `resolve`   | `query`, `total`, `showed?`, `results[]` (uuid, title, path, filetags, aliases, has_todos) |
| `suggest`   | `target`, `target_uuid`, `total`, `showed?`, `suggestions[]` (uuid, title, score, scores{}, reasons[], target_uuid) |
| `query`     | `query`, `total_results`, `showed?`, `results[]` (uuid, title, score, matches[], content_matches[]) |
| `context`   | `target`, `context`, `estimated_tokens`, `depth` |
| `validate`  | `uuid`, `title`, `path`, `filetags`, `categories`, `aliases`, `refs`, `headings`, `heading_uuids[]`, `outgoing`, `incoming`, `outgoing_internal`, `broken_internal[]`, `broken_files[]`, `backlinks[]` (uuid, title), `issues[]` (incl. self-links, overlinking), `healthy` |
| `orphans`   | `count`, `showed?`, `orphans[]` (uuid, title, path, filetags) |
| `get`       | `node` (uuid, title, path, filetags, categories, content?, headings?, headings_count?), `neighbors` |
| `path`      | `from`, `to`, `found`, `hops`, `path[]` (uuid, title) |
| `fix`       | `broken_uuid`, `replacement_uuid`, `replacement_title`, `files_affected[]`, `total_replacements`, `applied` |
| `new`       | `uuid`, `filename`, `path`, `title`, `created` |
| `info`      | `config`, `config_path`, `cli_overrides` |
| `agenda`    | `total`, `items[]` (uuid, title, path, filetags, has_agenda_tag, is_daily_file, daily_file_date?, heading_title, heading_level, todo_state?, priority?, scheduled?, scheduled_date?, deadline?, deadline_date?, is_overdue, heading_tags[]) |
| `todo`      | `total`, `items[]` (uuid, title, path, filetags, has_agenda_tag, is_daily_file, daily_file_date?, heading_title, heading_level, todo_state?, priority?, scheduled?, scheduled_date?, deadline?, deadline_date?, is_overdue, heading_tags[]) |

## Testing patterns

- **Unit tests** live in each module under `#[cfg(test)] mod tests { ... }`
- **Integration tests** in `tests/integration/` spawn the actual binary with a temp mock DB (one file per subcommand)
- **Property-based tests** in `graph.rs` and `parser.rs` use `proptest` (random Graph::Build fuzzing, slug roundtrip, UUID format, panic fuzzing)
- **JSON output testing**: All commands are tested with `--output-format json` via `test_all_commands_json`, verifying valid JSON output for every command
- Mock DB helper in `tests/integration/mod.rs::setup_db()` creates a 12+ note graph with duplicate UUIDs, broken links, filetags, aliases, and headings

## Feature flags

| Feature | Default | Description                              |
|---------|---------|------------------------------------------|
| `embed` | off     | Enables `--embed` flag in `query` and `suggest` commands for embedding-based semantic similarity via `fastembed`. Build with `--features embed`. |

## CLI flags

| Flag              | Description                                      |
|-------------------|--------------------------------------------------|
| `--output-format FMT` | Output format: `text`, `json`, or `ndjson`   |
| `--from-stdin`    | Read UUIDs from NDJSON stdin (Get, Suggest, Validate, Context) |
| `--headings`      | Show heading structure (Get only)                            |
| `--stats`         | Show database statistics (Check only)                        |
| `--file-links`    | Check file: link targets exist on disk (Check only)          |
| `--attachment-links` | Check attachment: link targets exist on disk (Check only) |
| `--id-links`      | Check id: link targets exist in the database (Check only)    |
| `--filetags`      | Check filetags format correctness (Check only)               |
| `--agenda`        | Check for planned TODO headings missing :agenda: filetag (Check only)|
| `--self-links`    | Check for self-referencing id: or file: links (Check only)          |
| `--overlinks`     | Check for 2+ internal links to the same note (Check only)           |
| `--cross-links`   | Check bidirectional links between two notes (Check only)            |
| `--todos`         | Show TODO/DONE statistics (Stats only)                       |
| `--todos`         | Restrict to files with TODO headings (Query, Resolve)        |
| `--missing-agenda`| Only show items missing :agenda: filetag (Agenda, Todo)      |
| `--include`       | Include only these TODO states (comma-separated) (Agenda, Todo)|
| `--exclude`       | Exclude these TODO states (comma-separated) (Agenda, Todo)    |
| `--overdue`       | Only show overdue items (Agenda only)                        |
| `--date`          | Show items scheduled/due on specific date (Agenda only)      |
| `--sort`          | Sort: priority, scheduled, deadline, file (Agenda); priority, state, file (Todo) |
| `--today`         | Show today's agenda items (Agenda only)                      |
| `--week`          | Show this week's agenda items (Agenda only)                  |
| `--limit`         | Maximum results (Agenda, Todo)                                |


## Command pipelining

Commands can be chained via Unix pipes using NDJSON:

```
pkms query "topic" --output-format ndjson | pkms get --links
pkms resolve --tags "ai" --output-format ndjson | pkms get --links
pkms stats --hubs --output-format ndjson | pkms get --links --no-content
```

Consumer commands (`get`, `suggest`, `validate`, `context`) accept `--from-stdin`
to read UUIDs from stdin. They auto-detect piped stdin when no target is given.

NDJSON output emits one JSON object per line (each with a `uuid` field).

- Producers: `resolve`, `query`, `orphans`, `stats --hubs`, `suggest`
- Consumers: `get`, `suggest`, `validate`, `context`

## Related

The `skill/pkms-manager/` directory contains an AI agent skill for efficiently using the pkms tool. See `SKILL.md` there for usage workflows.
