# Code Quality Improvement Plan

## ~~Critical: Code Duplication~~ ✅

1. ✅ **`todo.rs` / `agenda.rs` shared functions** — 9 of 10 shared functions extracted into `task_common.rs`. The `format_rows`/`filter_row` table helpers unified via `RowItem` trait. `print_agenda_text` and `print_todo_text_grouped` remain separate due to structural differences (sectioned vs flat layout).

2. ✅ **File link resolution** — Unified into `graph/mod.rs::resolve_file_link_path` and `file_link_target_exists`.

3. ✅ **UUID regex** — `ID_PROPERTY_RE` and `UUID_FORMAT_RE` centralized in `parser.rs`.

4. ✅ **`cfg(embed)` duplication** — Eliminated in `main.rs` using `#[cfg]` on pattern destructuring with conditional `let` shadowing.

## ~~High: Large/Monolithic Functions~~ ✅

| Function | Status |
|----------|--------|
| `dispatch()` (main.rs) | ⚠️ ~418 lines, inherently large due to match on all commands. Stdin-reading helper already extracted (`resolve_targets`, `read_stdin_ndjson`). |
| `Graph::build()` | ✅ Refactored into `BuildContext` struct. |
| `parse_note()` | ✅ Refactored into `ParseContext` struct. |
| `compute_scores()` | ✅ Split into per-score helpers. |
| `validate_one()` | ✅ Split into `check_*` helpers. |
| `run()` (todo) | ✅ Partially reduced by extracting `collect_todo_items`, `sort_items`, `print_todo_text`. |

## ~~Medium: Magic Numbers~~ ✅

- ✅ Scoring weights in `suggest.rs` — named constants (`TITLE_OVERLAP_WEIGHT`, etc.)
- ✅ Scoring weights in `search.rs` — named constants (`TITLE_MATCH_WEIGHT`, etc.)
- ✅ Column layout in `output.rs` — named constants (`BASE_PADDING`, `TAG_WEIGHT`, etc.)
- ✅ `50` word limit in `suggest.rs` — `MAX_CONTENT_KEYWORDS`

## ~~Medium: Structural Issues~~ ✅

5. ✅ `process_headings()` — converted to `BuildContext` method, 3 params. `print_check_json`/`print_check_text` — bundled into `CheckData` + `CheckDisplayOptions`.
6. ✅ `search_content()` — `seen_paths` HashSet filters duplicates.
7. ✅ `render_template()` — now uses `handlebars` crate.
8. ✅ `title_to_slug()` — regex-replace chain.
9. ✅ `config.rs` — `unwrap_or(PathBuf::from("."))`.

## ~~Low: Minor Concerns~~ ✅

10. ✅ Dead code removed (`all_categories()`, unused `_config` param).
11. ✅ Module-level doc comments on `graph/mod.rs`, `parser.rs`, `commands/mod.rs`.
12. ✅ `unwrap()` replaced with `expect("midnight is valid")` on `and_hms_opt`.
13. ✅ `parse_org_date()` — recursion depth guard (`MAX_PARSE_DEPTH = 32`).
14. ✅ `strip_org_links()` — safe `caps.get(1)` instead of `&caps[1]`.
15. ✅ `type_` renamed to `kind` (CLI flag preserved as `--type` via `long = "type"`).
16. ✅ `roam_aliases` renamed to `aliases` for consistency with `Node.aliases`.
17. ✅ Proptest moved to `new.rs`.
18. ⏭️ `format_size()` — kept as-is (12-line function, crate dependency not warranted).

## Summary

The codebase has a clean architecture with consistent command patterns and excellent test coverage. The main quality problems are:

1. **Code duplication** between `todo.rs`/`agenda.rs` (most impactful)
2. **Large functions** that should be decomposed
3. **Magic numbers** scattered in scoring/formula logic
4. **Duplicated link resolution** and **regex constants** across modules

No security vulnerabilities or unsafe code. No panics in normal operation.
