# TODO.md — pkms improvement roadmap

## Priority legend

- **P0** — Blocking, must fix for correctness or basic usability
- **P1** — High impact: AI agent usability, developer experience, test coverage
- **P2** — Medium: ergonomics, performance, code quality
- **P3** — Nice to have: polish, minor features, refactoring

---

## P0 — Critical fixes

- [x] **Fix typo in `new` command help text** (`src/commands/new.rs`): `"Actualy write the boilerplate file"` → `"Actually write the boilerplate file"`. Trivial but looks unprofessional.
- [x] **`query` command missing rayon parallel parsing**: Unlike `Graph::load()` which uses `into_par_iter()`, `query.rs` calls `into_iter()` in its manual build path (~line 55). Fix to use `into_par_iter()` with a `.collect::<Vec<_>>()`.
- [x] **`query` command missing `verbose` support**: The manual `Graph::build()` path in `query.rs` ignores the `verbose` parameter. Add progress logging consistent with `Graph::load()`.

---

## P1 — AI agent usability (highest priority)

These directly impact how effectively AI agents (and power users) can use pkms programmatically.

### Output / JSON reliability

- [x] **Guarantee pure JSON output with `--json`**: Audit every command to ensure `--json` produces **only** valid JSON on stdout, with no human-readable text mixed in. Currently `query` prints "Query: ..." and "Results: ..." to stdout in JSON mode. All informational text should go to stderr via `eprintln!`.
- [x] **Add `--quiet` / `-q` flag**: Suppresses all non-essential stderr output. AI agents parsing stdout don't want to see progress messages. Make `--json` imply `--quiet`.
- [x] **Structured error output in JSON mode**: When `--json` is active and a command fails, output `{"error": "message"}` to stdout (or stderr) instead of raw `anyhow` error text. Currently errors are just `bail!()` which produces non-JSON output even with `--json`.
- [x] **Exit code consistency**: Document and enforce a consistent exit code scheme. Currently only `check` uses non-zero exit codes (for unhealthy). Consider: 0 = success, 1 = business-logic failure (e.g., note not found, check unhealthy), 2 = parse/argument error.

### Machine-parseable output

- [x] **Add `--output-format` flag** (or `--csv`, `--ndjson`): For commands that list items (`broken`, `orphans`, `hubs`, `resolve`, `query`, `tags`), support simple line-based formats that are easier to pipe through shell tools. NDJSON (one JSON object per line) is ideal for AI scripts.
- [x] **`resolve` should accept `--json` for machine use**: Currently `resolve` only outputs JSON when `--json` is used. But its human output is already structured as a table; make the JSON output a proper `Vec<ResolveEntry>` with UUID, title, path, filetags.
- [x] **`resolve` output fields**: Add `--fields uuid,title,path,tags` to select columns. Default all for human, explicit for scripts.
- [x] **Consistent target resolution across commands**: Every command that accepts a `<target>` should produce the same error format when the target is not found. Create a shared `resolve_target_or_die()` helper function.

### Context & AI integration

- [x] **`context` — improve token estimation**: The current heuristic (`word_count * (1.0 + avg_word_len / 10.0)`) is crude. Use a character-based estimate (tokens ≈ chars / 4 for English) or add a `tiktoken-rs` integration. Document the estimation formula.
- [x] **`context` — add `--include-outgoing` / `--include-incoming` flags**: Currently `--depth` controls both. Sometimes you only want backlinks or only outgoing.
- [x] **`context` — add `--template` flag**: Allow a custom prompt template string with `{{title}}`, `{{content}}`, `{{neighbors}}`, `{{backlinks}}` placeholders.
- [x] **`suggest` — add `--json` output for programmatic consumption**: Already has `Serialize`, verify JSON output from `--json` works correctly and contains all scoring factors.
- [x] **`suggest` — expose per-factor scores in output**: So AI agents can understand WHY a note was suggested. Add `scores: HashMap<String, f64>` to the output struct.

### CLI for automation

- [x] **Add `--no-header` flag**: For `resolve`, `tags`, `broken`, `orphans`, `hubs`, suppress column headers in human output. Easier to `grep`/`cut`/`awk`.
- [x] **Add `--count` flag**: For list commands, only show the count of results, not the list itself.
- [x] **Support piping UUIDs**: Design a "batch" mode where commands accept UUIDs from stdin. E.g., `echo "aaaa-bbbb" | pkms get --depth 2 --from-stdin`. Or `pkms get --from-file /tmp/uuids.txt`.
- [x] **JSON input for targets**: Support `--input-json <file>` for passing complex query parameters (e.g., multiple targets, filter criteria) as JSON instead of CLI args.
- [x] **Add a JSON schema reference**: Create a JSON Schema file for every command's output. AI agents can use this to validate and parse responses reliably.
- [x] **Add `--example` flag to commands**: Each command should be able to print a usage example: `pkms path --example` → `pkms path "Note A" "Note B"`.
- [x] **Update AGENTS.md with JSON output schemas**: Document the exact JSON structure for each command so AI agents can reliably parse them.

---

## P1 — Testability

- [x] **Add integration tests for remaining commands**: `fix`, `suggest`, `resolve`, `subgraph`, `validate` (json), `tags --tag`, `stats --days`, `get --graph`, `get --out`, `context --max-tokens`.
- [x] **Add error-path integration tests**: Note not found, invalid UUID format, missing `--db` argument, empty database, malformed `.org` files.
- [x] **Add JSON schema validation in tests**: For `--json` output, parse with `serde_json` and verify struct fields are present and correctly typed, not just string-contains checks.
- [x] **Add snapshot testing**: Use `insta` crate for golden-file testing of human-readable output. Protects against accidental format changes that AI agents might rely on.
- [x] **Add property-based tests for graph operations**: Fuzz `Graph::build()` with random `FileScanResult` vectors, ensure no panics. Similarly for `find_node()` with edge-case inputs.
- [x] **Test duplicate UUID detection**: Create a test DB with duplicate UUIDs and verify `check` reports them. Currently untested.
- [x] **Test duplicate title detection**: Same as above.
- [x] **Test the `--json` flag on ALL commands**: A single parameterized test that runs every command with `--json` and asserts the output is valid JSON. Use the mock DB.
- [x] **Extract shared test helpers**: Move `setup_db()` into a reusable test module. Add helper functions like `run_pkms(&["--json", "check"])` that return `(stdout, stderr, exit_code)`.

---

## P2 — Architecture & code quality

- [ ] **Decouple parsing from graph building**: `parser.rs` returns `ParsedNote` with raw strings; `graph.rs::build()` converts to `Node`. Extract a `Node::from_parsed()` constructor. The current `Graph::build()` does too much (duplicate detection, backlink construction, broken link detection, node creation).
- [ ] **Add an alias index to `Graph`**: Replace O(n) linear scan in `find_node()` with a `HashMap<String, Vec<String>>` (alias → UUIDs). This matters at scale (750+ notes, each with multiple aliases).
- [ ] **Remove `#[allow(dead_code)]` where possible**: Either use the fields or remove them. `Node.id` exists but is never accessed externally (auto-increment counter exists only for potential future use). `Node.content_hash` is stored but never compared. `Link::File`, `Link::Url`, `Link::Attachment` variants are parsed but never consumed (only counted in stats).
- [ ] **Standardize serialization**: `get.rs` and `subgraph.rs` build `serde_json::Value` manually. Convert to derive-based `Serialize` structs like every other command.
- [ ] **Reduce cloning in command code**: Many commands call `.cloned()` on `graph.nodes`. Prefer returning references or use `Arc` for shared data. At minimum, add a benchmark to measure the cost.
- [ ] **`find_node_exact()` is unused**: Either remove it or use it in the relevant command path. Currently `#[allow(dead_code)]`.
- [ ] **Add `deny_unknown_fields` to config structs**: `#[serde(deny_unknown_fields)]` on the config deserialize struct to catch typos in `~/.config/pkms.toml`.
- [ ] **Inconsistent import style**: Some files `use crate::graph::Graph; Graph::...` others `use crate::graph; graph::Graph::...`. Pick one convention (prefer the shorter one) and enforce.
- [ ] **Lint setup**: Add `[lints]` section to `Cargo.toml` with `clippy::pedantic` or a curated subset. Create a CI workflow that runs `cargo clippy` and `cargo fmt --check`.

---

## P2 — Performance

- [ ] **Graph caching**: Serialize the built graph to disk (e.g., `~/.cache/pkms/graph.bincode` or JSON) so subsequent commands in quick succession skip the 3s full rebuild. Invalidate cache when files change (check `content_hash` or mtime).
- [ ] **Incremental parsing**: Only re-parse files whose mtime has changed since last build. Store timestamps alongside the cached graph.
- [ ] **Add a `--no-content` flag to `Graph::load()`**: For commands that don't need full content (e.g., `stats`, `orphans`, `broken`, `hubs`), skip storing file contents in memory. Currently `Node` stores the full file content only in `get --out` and `context`; but the content is kept in the `ParsedNote` → `FileScanResult` pipeline.
- [ ] **`resolve` command is already fast (~0.5s)**: Document it as the recommended first step for AI agent workflows. Ensure it stays fast by keeping its header-only scan pattern.
- [ ] **Parallel file I/O**: Currently file reading happens in a `par_iter()` but each file read is synchronous. Use `tokio` + async reads for non-blocking I/O when scaling to 10k+ files (low priority).

---

## P2 — Missing features

- [ ] **`check` — validate file links**: Currently `check` only counts broken internal links. Add `--verify-file-links` flag that checks if `file:` targets exist on disk.
- [ ] **`check` — validate attachment links**: Same for `attachment:` links.
- [ ] **`clean` / `gc` command**: Remove notes that have no UUID, no links to them, and haven't been modified in N days. Orphan cleanup.
- [ ] **`export` command**: Export the graph (or a subgraph) to GraphML, GEXF, or DOT format for visualization in Gephi/Graphviz.
- [ ] **`rename` / `move` command**: Rename a note's title and/or path, updating all links pointing to it.
- [ ] **`merge` command**: Merge two notes (combine content, redirect links, deduplicate). Handle duplicate UUID resolution.
- [ ] **`diff` command**: Compare two notes or two versions of the same note.
- [ ] **`watch` / `daemon` command**: File watcher that auto-rebuilds the graph cache on file changes. Useful for REPL-style AI agent workflows.
- [ ] **`alias` subcommand**: Manage aliases for a note (list, add, remove).

---

## P3 — Polish & minor

- [ ] **Colorized output**: Add optional color with `--color` flag (auto/always/never) using `termcolor` or `colored`. Different colors for UUIDs, titles, paths, scores.
- [ ] **Progress bars**: Use `indicatif` for long-running commands (`check`, `stats` on large DBs). Gated behind `--verbose` or no `--quiet`.
- [ ] **Shell completion**: Add `clap_mangen` / `clap_complete` to generate shell completions. Add `pkms completions bash > /etc/bash_completion.d/pkms`.
- [ ] **`init-config` — add `--overwrite` flag**: Currently errors if config exists. Add `--overwrite` to replace.
- [ ] **Man page**: Add a man page generation step. `pkms man` → outputs troff.
- [ ] **Add tests for `init-config`**: Currently no tests for config initialization.
- [ ] **Benchmark suite**: Add `criterion` benchmarks for critical paths: `Graph::load()`, `find_node()`, `search()`, `search_content()`. Run as `cargo bench`.
- [ ] **Dockerfile**: Containerize the tool for CI/CD and isolated AI agent execution.
- [ ] **Nix flake / Homebrew formula**: Distribution packages for easy installation.
- [ ] **README.md references TODO.md**: Backfill the reference now that this file exists. Update README to link here.
