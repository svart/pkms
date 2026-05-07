# pkms — Development Roadmap & Improvement Plan

## Priority Legend

- 🔴 **Critical** — bug or broken user-facing behavior
- 🟡 **Important** — material quality/structure issue
- 🟢 **Minor** — polish, idiomatic cleanup
- 🔵 **Future** — new feature / strategic target

---

## 🔴 Critical Issues

### 1. `unreachable!()` panic in `src/main.rs:224`

The `dispatch_query` function uses `unreachable!()` in its catch-all arm. If a new `Command` variant is added to the enum but not handled in `dispatch_mutating`, the program panics instead of returning a clean error.

**Fix:** Replace with `anyhow::bail!("unhandled command: {cli.command:?}")`.

### 2. README references non-existent `TODO.md`

`README.md:234` links to `./TODO.md` which doesn't exist. Either create this file or remove the reference.

**Fix:** This file is the fix.

### 3. `fix` replaces UUIDs everywhere, not just in links

`src/commands/fix.rs:84` uses `content.replace(broken_str, replacement_uuid)` which replaces the UUID anywhere it appears in the file — including in prose, property drawers, or other non-link contexts. It should only replace within link brackets (`[[id:...][...]]`) and bare `id:...` references.

**Fix:** Use a regex or targeted replacement that matches only `[[id:<broken>][` and `id:<broken>` patterns.

### 4. DIY mustache template in `context.rs` is fragile

The hand-rolled template engine at `src/commands/context.rs:178-216` has edge cases:
- Unclosed `{{#tag}}` leaves raw tags in output
- Values containing `{{/tag}}` literally trigger premature close-tag matching
- The `while let` loop has no upper bound guard

**Fix:** Replace with the `handlebars` crate, or at minimum add input validation (reject templates with unmatched conditionals).

---

## 🟡 Important Structural Improvements

### 5. Three-tier dispatch in `main.rs` is overly complex

The chain `dispatch → dispatch_simple → dispatch_complex → dispatch_mutating → dispatch_query` (4 levels deep, `src/main.rs:48-226`) makes the control flow hard to follow. Each level returns `Option<ExitCode>` or `Result<ExitCode>`.

**Fix:** Flatten to a single `match cli.command` with early returns. Move `init-config` to `src/commands/init_config.rs` to eliminate the inline special case.

### 6. Duplicated file-walking logic in `resolve.rs`

`src/commands/resolve.rs:42-125` (`scan_files`) reimplements the same `WalkDir` + ignore filtering + `.org` extension check already present in `src/discovery.rs:10-43`. The only difference is that resolve parses only the header (first 100 lines), while discovery reads the whole file.

**Fix:** Extract a `walk_org_files(root, ignore) -> impl Iterator<Item=PathBuf>` in `discovery.rs` and use it from both `discover_files()` and `resolve::scan_files()`.

### 7. `resolve_db_root` called in validate inner loop

`src/commands/validate.rs` calls `config.resolve_db_root(db_cli)` inside `validate_one()`, which is invoked once per piped target. When an agent pipes 50 notes through validate, the db_root is resolved 50 times identically.

**Fix:** Resolve db_root once in `validate::run()` and pass it as a parameter.

### 8. Missing `init-config` skill reference file

Every command has a dedicated reference in `skills/pkms-manager/references/<name>.md`, but `init-config` is documented only inline in the SKILL.md without its own reference file.

**Fix:** Either add `skills/pkms-manager/references/init-config.md` following the pattern, or remove the expectation.

---

## 🟢 Idiomatic Code Cleanup

### 9. All `Graph` fields are `pub`

`src/graph/mod.rs:65-73`: `nodes`, `path_to_uuid`, `title_to_uuid`, `alias_to_uuid`, `backlinks`, `broken_links`, `parse_errors`, `skipped_files`, `duplicates` are all `pub`. Tests mutate `graph.nodes.get_mut("b")` directly.

**Fix:** Make fields `pub(crate)`. Provide accessor methods for immutable reads.

### 10. Redundant `apply` parameter in `print_fix_output`

`src/commands/fix.rs:23`: Function accepts both `&FixOutput` (which has `output.applied: bool`) and a separate `apply: bool` parameter. They carry the same information.

**Fix:** Remove the `apply` parameter; use `output.applied` directly.

### 11. Dense boolean formulas in `check.rs`

`src/commands/check.rs:98-104`:
```rust
let show_id = id_links || !(file_links || attachment_links || filetags);
```

This is correct but requires mental parsing to understand "show X if explicitly requested OR if nothing else was requested."

**Fix:** Extract helper:
```rust
let any_explicit = file_links || attachment_links || filetags;
let show_id = id_links || !any_explicit;
```

### 12. Unused `_backlink_entries` parameter in validate

`src/commands/validate.rs:75`: The `_backlink_entries: &[BacklinkEntry]` parameter in `print_validate_text` is unused. It was presumably kept for future output formatting.

**Fix:** Remove it (it's been unused since implementation, and `output.backlinks` is already available to the caller).

### 13. Token counting logic duplicated in `context.rs`

`estimate_tokens` (line 234) and `truncate_by_tokens` (line 253) both implement the same token-counting loop with identical logic.

**Fix:** Extract a shared helper `fn count_tokens_up_to(text: &str, max: Option<usize>) -> (usize, Option<usize>)` that returns (total_tokens, truncation_position).

### 14. `format_size` double-computes divisor in `stats.rs`

`src/commands/stats.rs:262-279`: The function first divides `scaled /= 1024` in a loop, then recalculates the divisor with a match statement on the resulting unit. The divisor is deterministic from the loop count.

**Fix:** Track `divisor` during the loop: `divisor *= 1024` (starting from 1).

### 15. Dead code in `title_to_slug` in `new.rs`

`src/commands/new.rs:94-107`: The `' '` case is matched explicitly, but it's also caught by the `'_' | ' ' | '-'` arm earlier in the same match. This is not a bug (the early arm wins), but the explicit `' '` will never match.

**Fix:** Remove the standalone `' ' => '_'` arm.

### 16. `query.rs` uses `scan()`+`build()` instead of `Graph::load()`

`src/commands/query.rs:51-54`: Calls `Graph::scan(&db_root, &ignore)?` then `Graph::build(results)` instead of `Graph::load(config, db_cli)`. Same result, inconsistent pattern.

**Fix:** Use `Graph::load()` for consistency with all other commands.

---

## 🔵 Strategic Targets (AI Agent Usage)

### 17. JSON Schema for all command outputs

Every command supports JSON output, but agents must either hardcode the shape or read docs. A `--schema` flag (or `pkms schema <command>`) emitting JSON Schema via `schemars` would let agents validate and navigate outputs programmatically.

**Value:** Very high for agent reliability. ~200 lines of integration.

### 18. Accurate token counting via tiktoken-rs

The current `estimate_tokens` uses a naive word-count heuristic that can be 30%+ off for code-heavy notes. Integrating `tiktoken-rs` with `--encoding` flag (cl100k_base, o200k_base, etc.) would give exact token counts matching the target LLM.

**Value:** High for agents working within context limits. Critical for `context --max-tokens`.

### 19. Customizable `context` template

The template is hardcoded in `src/commands/context.rs:10`. Different LLMs benefit from different formatting:
- Claude: XML tags
- GPT: markdown sections
- DeepSeek: plain text with clear delimiters

**Fix:** Add `--template` / `--template-file` flags. Must fix issue #4 (DIY template engine) first — switch to `handlebars` crate, then expose template customization.

### 20. Section-level content extraction

`get` returns the full note. An agent often wants a specific heading subtree (e.g. "Implementation Details" only). A `--heading "Section Name"` filter on `get` would save tokens and focus attention.

**Pre-requisite:** The parser already extracts headings and their levels. This is display-only work in `get.rs`.

### 21. Embedding-based similarity for `suggest` and `query`

Current search is substring-only. For an agent looking for conceptually related notes (e.g., "exception safety" → "RAII", "error handling"), substring matching misses. A lightweight option using `fastembed` or a local embedding server to compute on-the-fly semantic similarity would dramatically improve relevance.

This is NOT RAG — it's a better similarity metric for the existing `suggest` and `query` commands, respecting the stateless model (compute fresh each run).

### 22. `link` command for minimal note editing

Currently only `fix` modifies files. An agent that discovers a connection between two notes must `sed` the file or edit manually. A simple:
```bash
pkms link <source> <target> --description "text"
```
that appends `[[id:<target-uuid>][text]]` to the source note would be the right balance — targeted editing, not a "clever changer."

### 23. `--since` filter for incremental scan

Scanning 10k+ files for every `get` or `validate` call is wasteful in agent workflows like "query → validate → get → suggest." A `--since <timestamp>` filter preserves statelessness (no cache) while letting agents of the "search → verify → explore" pattern run faster. The agent controls the timestamp.

### 24. DOT/Graphviz export for `path`

`pkms path A B --output-format dot` producing a Graphviz graph would let agents visualize connection pathways and explain graph topology to users.

### 25. Agent self-discovery — `pkms info --capabilities`

An agent's first interaction should tell it what the tool can do. A `--capabilities` flag on `info` (or dedicated `capabilities` command) that returns a JSON object listing all commands, their flags, argument types, and output schema URLs would let agents bootstrap without reading SKILL.md each time.

### 26. Incremental `--files` flag for targeted scanning

Instead of full rescans, allow the agent to specify changed files directly:
```bash
pkms get --files note1.org note2.org <target>
```
This tells the graph loader to only scan those specific files, falling back to a full scan for the rest of the graph. Useful when the agent just created/edited two notes and wants to validate them.

---

## Implementation Order

### Phase 1 — Fix bugs (do first)
- #1 `unreachable!()` panic
- #2 Missing TODO.md 
- #3 `fix` replaces UUIDs outside links
- #4 DIY template engine fragility

### Phase 2 — Structure cleanup
- #5 Flatten dispatch
- #6 Deduplicate file-walking
- #7 Move `resolve_db_root` out of validate loop
- #8 Add missing `init-config` reference file

### Phase 3 — Idiomatic polish
- #9 Graph field visibility
- #10 Redundant parameter in fix
- #11 Clean up check boolean logic
- #12 Remove unused validate parameter
- #13 Deduplicate token counting
- #14 Simplify format_size
- #15 Clean up title_to_slug
- #16 Use Graph::load in query

### Phase 4 — Agent features
- #17 JSON Schema output
- #18 Accurate token counting
- #19 Customizable context template
- #20 Section-level content extraction
- #21 Embedding-based similarity
- #22 `link` command
- #23 `--since` filter
- #24 DOT export for path
- #25 `info --capabilities`
- #26 `--files` for targeted scanning
