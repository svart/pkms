# PKMS Code Quality & Architecture Improvement Plan

## Progress

| Phase | Status | Date |
|-------|--------|------|
| Phase 1: Reduce Code Duplication | ✅ Complete | 2026-05-02 |
| Phase 2: Split Monolithic Modules | ✅ Complete | 2026-05-02 |
| Phase 3: Fix Inconsistencies | ✅ Complete | 2026-05-02 |
| Phase 4: Improve Data Model & Performance | ✅ Complete (4.3 only) | 2026-05-02 |
| Phase 5: Improve Test Infrastructure | ✅ Complete (5.1 only) | 2026-05-02 |
| Phase 6: Documentation & Tooling | ✅ Complete (6.2, 6.3) | 2026-05-02 |

## Summary

The pkms project is a well-functioning CLI tool with 18 commands, ~4,400 lines of Rust,
strong test coverage (79 integration tests + property-based fuzzing), and clean conventions.
This plan identifies 18 concrete improvements organized by priority and effort.

---

## Priority 1: Reduce Code Duplication (Low Effort, High Impact)

### 1.1 Extract `input_json` target loading into a shared helper

**Problem:** 4 commands (`validate`, `suggest`, `subgraph`, `context`) contain identical
9-line blocks for reading `--input-json` and extracting a `target` field. `query` has a
near-identical variant for `terms`. Meanwhile `get` and `path` offload this to `main.rs`
helper functions — creating an inconsistency.

**Resolution:** Removed `--input-json`, `--from-stdin`, and `--from-file` flags entirely
(2026-05-03). These automation-oriented flags were rarely used and added unnecessary
complexity. All commands now accept targets directly as positional arguments.

### 1.2 Extract `count_only` early-return pattern

**Problem:** 6 commands contain identical 8-line blocks:

```rust
if count_only {
    if json { println!("{}", serde_json::json!({"count": count})); }
    else { println!("{}", count); }
    return Ok(());
}
```

**Fix:** Create `fn print_count(count: usize, json: bool) -> Result<()>` in a shared
utility module. Replace all 6 instances.

### 1.3 Extract ndjson output pattern

**Problem:** 5 commands contain identical ndjson printing loops:

```rust
if ndjson {
    for e in &entries {
        println!("{}", serde_json::to_string(e)?);
    }
}
```

**Fix:** Create `fn print_ndjson<T: Serialize>(entries: &[T]) -> Result<()>`.

### 1.4 Deduplicate walkdir + regex in `resolve.rs`

**Problem:** `resolve.rs` has its own `scan_files()` (69 lines) with its own
`UUID_RE`, `TITLE_RE`, `FILETAGS_RE`, `ALIASES_RE` statics that partially duplicate
`parser.rs`. These regexes and the walk logic will inevitably diverge from `parser.rs`
when org-mode parsing rules change.

**Fix:** Refactor `parser.rs` so a single pass over the first 100 lines extracts exactly
what `resolve` needs — expose a `fn parse_file_header(content: &str) -> HeaderInfo`
function. `resolve::scan_files()` uses `discovery::discover_files()` + this new function,
eliminating duplicated walkdir and regex logic.

### 1.5 Remove orphan-detection logic duplication inside `graph.rs`

**Problem:** `orphan_nodes()` (line 588-598) and `stats()` (line 636-647) contain the
same 7-line orphan filter closure — a second copy of the same logic with a different
return type.

**Fix:** `stats()` calls `self.orphan_nodes().len()` instead of reimplementing the filter.

### 1.6 Extract repeated display patterns

**Problem:** Short UUID truncation (`&uuid[..8]`) appears 5 times. `path.to_string_lossy().to_string()`
appears 25+ times. `no_header` conditional header printing has 6 structurally identical instances.

**Fix:** Add these to a shared utility:
- `fn short_uuid(uuid: &str) -> &str`
- `fn path_string(path: &Path) -> String`
- `fn print_header<F: FnOnce()>(no_header: bool, printer: F)` or a `HeaderPrinter` struct

---

## Priority 2: Split Monolithic Modules (Medium Effort, High Impact)

### 2.1 Split `graph.rs` (1064 lines) into submodules

**Problem:** One file handles graph construction, BFS traversal, search, analytics,
and filesystem queries. The single-file structure makes it hard to navigate and modify.

**Proposed split:**

```
src/graph/
  mod.rs           (~260 lines)  — Node, Graph, FileScanResult, supporting types,
                                   load(), find_node(), resolve_target(), re-exports
  builder.rs       (~116 lines)  — Graph::build() constructor
  traversal.rs     (~190 lines)  — get_neighbors(), find_shortest_path(),
                                   collect_subgraph() (all BFS-based)
  search.rs        (~90 lines)   — search(), search_content(), all_tags(), notes_by_tag()
  analytics.rs     (~80 lines)   — hubs(), orphan_nodes(), stats(),
                                   directory_breakdown(), disk_size(), notes_since()
```

Tests move to `tests/graph_tests.rs` or stay within each submodule as `#[cfg(test)]`.

### 2.2 Unify `query.rs` graph construction

**Problem:** `query.rs` calls its own `discover_files()` then parses all files with `rayon`
and calls `Graph::build()` manually — duplicating `Graph::load()`. The comment says this is
intentional, but the duplicated pipeline is ~30 lines of code
that will need maintenance whenever discovery or parsing changes.

**Fix:** Refactor `Graph::load()` into two phases:
1. `Graph::scan(config, db_cli, verbose)` — returns `Vec<FileScanResult>` (discovery + parse)
2. `Graph::build(scan_results)` — the existing constructor

Both `Graph::load()` and `query::run()` call `scan()` then `build()`. The `query` command
can then add content-aware scoring on top of the same graph instance. This also enables
reusing scan results between commands in the future (e.g., a daemon mode).

---

## Priority 3: Fix Inconsistencies (Low Effort, Low Impact)

### 3.1 Unify `input_json` handling

**Problem:** Half the commands handle `--input-json` internally, half have the parsing
done in `main.rs` helper functions. Some extract only `target`, others extract multiple
params. This inconsistency means developers must check each command individually.

**Resolution:** Removed `--input-json`, `--from-stdin`, and `--from-file` flags entirely
(2026-05-03). Deleted `load_get_params()` and `load_path_params()` from `main.rs` along
with `load_input_target()` from `util.rs`.

### 3.2 Unify `check` return type

**Problem:** `check::run()` returns `Result<bool>` while every other command returns
`Result<()>`. The `main.rs` dispatch has a special arm for Check with `.map(|healthy| ...)`.

**Fix:** Return `Result<()>` and embed the healthy status in the output struct.
Let `main.rs` read the output field to determine exit code — or have `check::run()`
return `ExitCode` via a wrapper identical to `init_config()`.

### 3.3 Add `--no-header` support to all commands

**Problem:** Only 6 of 18 commands support `--no-header`. Commands like `stats`,
`check`, `validate`, `suggest` don't.

**Fix:** Add `no_header: bool` to all command signatures and their clap definitions.
After Priority 1.6, this becomes a 1-line addition per command.

### 3.4 Remove or use unused parameters

**Problem:** `suggest::run()`, `stats::run()`, and `fix::run()` accept `verbose` but prefix it
`_verbose` (unused). `quiet` is only used by `context`.

**Fix:** Either remove unused parameters, or actually use them. For `suggest`:
print scoring breakdown when `verbose=true`. For `stats`: print per-directory
breakdown when verbose. For `fix`: print individual file replacements when verbose.

---

## Priority 4: Improve Data Model & Performance (Medium Effort, Medium Impact)

### 4.2 Avoid second walkdir scan in `fix.rs`

**Problem:** After `Graph::load()` already walks the entire DB, `fix.rs` does its own
`walkdir::WalkDir::new()` to find files containing the broken UUID (line 79-108).

**Fix:** During graph construction, or as a utility on Graph, store a
`HashMap<String, Vec<PathBuf>>` mapping each UUID to the files that reference it.
This enables O(1) lookup for `fix` instead of O(n) walk.

If storing all UUID->file mappings is too expensive, at minimum reuse the file
paths from `Graph::nodes` (already loaded). For each node, check if its outgoing links
contain the broken UUID — this is O(n) but avoids the second filesystem walk.

### 4.3 Improve token estimation accuracy

**Problem:** `context.rs` uses `chars.count() / 4` for token estimation. This can be
off by 30-50% depending on content (code blocks, CJK characters, whitespace, markdown).

**Fix:** Use a simple regex-based tokenizer mimicking common LLM tokenizer behavior:
```
tokens ≈ chars/4 for Latin text + 1 token per CJK character + 1 token per
newline or significant whitespace boundary
```
Even a 2-pass heuristic (chars/4 for ASCII, count for non-ASCII) would be better.

---

## Priority 5: Improve Test Infrastructure (Medium Effort, Moderate Impact)

### 5.1 Move graph tests out of source file

**Problem:** `graph.rs` tests occupy 346 lines (32% of the file). This makes the source
file harder to navigate and blurs the line between production and test code.

**Fix:** Create `tests/graph_tests.rs` and move all 17 unit tests + 2 proptest tests there.
Keep the public API surface tested via integration tests already present in `tests/integration.rs`.
This is consistent with community Rust practice for larger test suites.

### 5.2 Add snapshot tests for more commands

**Problem:** Only `broken`, `tags`, `path`, `resolve` have `insta` snapshot tests.
Commands like `stats`, `check`, `query` would benefit from snapshot tests to catch
regressions in output format.

**Fix:** Add `insta::assert_snapshot!()` tests for `stats`, `check`, `query`, `hubs`, `suggest`.

### 5.3 Add property-based tests for `fix` and `suggest`

**Problem:** `fix` (file writes) and `suggest` (complex scoring) have no fuzzing tests.
A bad regex replacement in `fix` or a NaN score in `suggest` could corrupt data.

**Fix:** Add proptest for `fix` using temporary files (already have `tempfile`).
Add proptest for `suggest` scoring to ensure scores are always finite and sorted correctly.

---

## Priority 6: Documentation & Tooling (Low Effort, Low Impact)

### 6.1 Create root `schemas/` directory

**Problem:** AGENTS.md and README.md reference `schemas/check.json`, `schemas/stats.json`,
etc., but those files only exist at `skill/pkms-manager/schemas/`. Commands added after
the skill was created (`tags`, `subgraph`, `path`, etc.) have no schema files anywhere.

**Fix:** Either:
- Create symlink `schemas` → `skill/pkms-manager/schemas/`
- Add schema files for commands currently missing them (`hubs`, `path`, `subgraph`, `tags`,
  `broken`, `orphans`, `validate`, `fix`, `new`, `info`)

### 6.2 Add `rustfmt` and `clippy` to CI/pre-commit

**Problem:** The AGENTS.md says "No lint or formatting commands are configured. The
project has no clippy or rustfmt CI." This means inconsistent formatting and missed
lint warnings.

**Fix:** Add `rustfmt.toml` (or default rustfmt), run `cargo fmt`, add `cargo clippy -- -D warnings`.
Document commands in AGENTS.md to make agent use them always before committing changes.

---

## Implementation Order (Recommended)

| Phase | Items | Why First |
|-------|-------|-----------|
| **Phase 1** | 1.1–1.6 (Duplication) | These are pure refactors with no behavioral changes. Safe, testable, immediate payoff. |
| **Phase 2** | 2.1 (Split graph.rs) | Unblocks easier modifications; graph is the most-changed module. |
| **Phase 3** | 3.1–3.4 (Inconsistencies) | Small changes that polish the API and remove developer surprises. |
| **Phase 4** | 2.2 (Unify query pipeline) | Depends on 2.1 graph refactor. |
| **Phase 5** | 4.2–4.3 (Performance) | Optional improvements; do these when they become bottlenecks. |
| **Phase 6** | 5.1–5.3 (Tests) | Better to have split modules first (5.1 depends on 2.1). |
| **Phase 7** | 6.1–6.3 (Docs/Tooling) | Low urgency but improves maintainability. |

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Refactoring graph.rs breaks 13+ commands | 79 integration tests cover every command's use of Graph |
| Split modules break imports | `pub use graph::*` re-exports from `graph/mod.rs` maintain backward compatibility |
| Content caching increases memory | Make it opt-in via `load_content` flag (default false) |
| Changes to `fix.rs` corrupt note files | Integration tests with temp DB; `fix` test already validates dry-run vs apply |

## Key Findings Summary

| Category | Count | Examples |
|----------|-------|---------|
| Duplicated code blocks | 7 patterns, 20+ instances | count_only, ndjson, short UUID |
| Duplicated regex statics | 4 | UUID_RE, TITLE_RE, FILETAGS_RE, ALIASES_RE in resolve.rs + parser.rs |
| Monolithic files >300 LOC | 3 | graph.rs (1064), resolve.rs (369), context.rs (318) |
| Commands missing features | 12 | no_header, unused verbose/quiet params |
| Missing schema files | 10 | hubs, path, subgraph, tags, broken, orphans, validate, fix, new, info |
| Unused parameters | 4 | verbose in suggest/stats/fix, quiet (only context uses it) |
