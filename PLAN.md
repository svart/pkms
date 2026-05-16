# Code Quality Improvement Plan

## Critical: Code Duplication

1. **`todo.rs` / `agenda.rs` — ~350 lines of identical code.** These two modules (813 + 729 lines) share at least 10 identical functions: `combine_tags`, `extract_date`, `is_overdue`, `parse_filters`, `apply_state_filter`, `apply_tags_filter`, `apply_type_filter`, `priority_value`, `format_display_datetime`, and the table-formatting skeleton. Extract into a shared `src/commands/task_common.rs` module.

2. **File link resolution logic is defined three times** in `check.rs:92-129`, `validate.rs:228-266`, and `graph/mod.rs:101-124` — all handling `org:` prefix, `~` expansion, `::` line specs, and relative/absolute path resolution. Consolidate into a single function on `Graph` and have all callers use it.

3. **UUID regex `:ID:\s+([a-f0-9-]+)` appears in 3+ files** (`validate.rs:13-19`, `resolve.rs:36`, `graph/mod.rs`). Make this a public constant in `parser.rs`.

4. **`#[cfg(feature = "embed")]` / `#[cfg(not(feature = "embed"))]` arms** duplicate ~30 lines each in `main.rs` (Suggest, Query dispatch) and again in `query.rs` and `suggest.rs`. Use a cfg-dependent local variable for `use_embed: bool` to eliminate the duplication.

## High: Large/Monolithic Functions

| Function | File | Lines | Issue |
|----------|------|-------|-------|
| `dispatch()` | `main.rs:81` | ~467 | Grows with every new command; extract stdin-reading helper |
| `Graph::build()` | `builder.rs:129` | ~191 | Three distinct phases that should be separate functions |
| `parse_note()` | `parser.rs:100` | ~175 | Mixed concerns: properties, headings, links, source blocks |
| `compute_scores()` | `suggest.rs:176` | ~165 | All scoring in one monolith; split by score category |
| `validate_one()` | `validate.rs:159` | ~195 | Too many checks in one function; split into `check_*` helpers |
| `run()` (todo) | `todo.rs:212` | ~263 | Collection, filtering, sorting, and output all in one function |

## Medium: Magic Numbers

- **Scoring weights in `suggest.rs`**: `20.0`, `5.0`, `25.0`, `15.0`, `12.0`, `5.0`, `100.0` — unnamed magic values
- **Scoring weights in `search.rs`**: `10.0`, `6.0`, `5.0` — same issue
- **Column layout in `output.rs`**: `5 + 2 * n_columns` (padding), `15` (min_col), weight coefficients `0.20`, `0.35`, `0.45` — should be named constants
- **`suggest.rs:393`**: `50` word limit for content keywords — unnamed

## Medium: Structural Issues

5. **`process_headings()` in `builder.rs:40`** has 9 parameters; `print_check_json()` and `print_check_text()` in `check.rs` have 11 and 10 parameters respectively — all suppressed with `#[allow(clippy::too_many_arguments)]`. Bundle into context structs.

6. **`search_content()` in `search.rs:105`** iterates `self.nodes.values()` which includes both primary nodes and heading-node duplicates, causing content hits to double-count. Should filter to unique paths.

7. **Hand-rolled template engine in `context.rs:169`** (`render_template()`) handles `{{key}}` substitution and `{{#key}}...{{/key}}` conditionals with manual string search/replace. Nested conditionals break silently. Consider using `handlebars` or `tera`.

8. **`title_to_slug()` in `new.rs:223`** uses per-character mapping via an explicit match; could be simplified with a regex-replace chain.

9. **`config.rs:119`** — `std::env::current_dir().unwrap_or_default()` — `current_dir()` is documented as potentially failing if the CWD was deleted. Use `.unwrap_or(PathBuf::from("."))`.

## Low: Minor Concerns

10. **Dead code**: `search.rs:123` `all_categories()` is `#[allow(dead_code)]` — either expose it via a CLI flag or remove it. `stats.rs:264` unused `_config: &Config` parameter.

11. **No module-level doc comments** on any `pub mod` file. The `Graph` struct and `FileScanResult`/`ParsedNote` relationship is undocumented.

12. **`unwrap()` on `.unwrap_or_default()` patterns** in production code (`main.rs:429,438`, `todo.rs:193,201,207`) for `and_hms_opt(0,0,0)` — always valid but fragile. Use `expect()` or a const `NaiveTime::MIN`.

13. **`parse_org_date()` in `org_date.rs:23`** has unbounded recursion on malformed input like `><<"—`. Add a recursion depth guard.

14. **`strip_org_links()` closure in `parser.rs:277`** captures `caps` by reference with subtle lifetime interactions.

15. **`type_` field name** (trailing underscore to avoid keyword) in `cli.rs`, `todo.rs`, `agenda.rs` — rename to `kind` or `item_type`.

16. **`roam_aliases` (parser) vs `aliases` (Node)** — inconsistent naming across modules.

17. **Proptest in `parser.rs`** references `super::super::commands::new::title_to_slug` — cross-module back-reference in a parser test for a command function.

18. **`format_size()` in `stats.rs:338`** — custom byte-size formatter; could use `humansize` or similar crate.

## Summary

The codebase has a clean architecture with consistent command patterns and excellent test coverage. The main quality problems are:

1. **Code duplication** between `todo.rs`/`agenda.rs` (most impactful)
2. **Large functions** that should be decomposed
3. **Magic numbers** scattered in scoring/formula logic
4. **Duplicated link resolution** and **regex constants** across modules

No security vulnerabilities or unsafe code. No panics in normal operation.
