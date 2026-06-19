# Code Review Fix Plan

This plan tracks fixes from the repository-wide code review. Work through it in
order unless a later task is clearly independent. Each task should land as a
small, focused change with tests near the affected behavior.

## Phase 1: Serve Asset Access

Goal: prevent `pkms serve` from exposing arbitrary files under the user's home
directory through `/asset`.

- [x] Confirm the intended serve asset policy:
  - [x] `attachment:` targets may resolve only through the supported org-attach
        layouts for the note UUID.
  - [x] `file:` targets may resolve only if the note actually contains the same
        parsed file link target, or only under `db_root` if that is the desired
        stricter policy.
  - [x] No request should be allowed just because the resolved path is under
        `dirs::home_dir()`.
- [x] Replace the broad home-directory allowlist in
      `src/commands/serve/assets.rs`.
- [x] Validate `/asset` requests in `src/commands/serve/http.rs` against the
      note's parsed outgoing file or attachment links before reading from disk.
- [x] Add serve tests covering:
  - [x] A linked DB-local file or image is still served.
  - [x] A linked attachment is still served.
  - [x] An unlinked file under the home directory is rejected.
  - [x] Path traversal and absolute-path variants are rejected.
- [x] Run focused checks:
  - [x] `cargo test --features web serve`
  - [x] `cargo test --features web --test integration serve`

## Phase 2: Org Parser Case Handling

Goal: make parser behavior consistent with common org-mode keyword casing.

- [x] Add small parser helpers for case-insensitive org keyword and drawer
      marker matching.
- [x] Make source block detection case-insensitive for both begin and end
      markers in `src/parser.rs`.
- [x] Make `#+filetags:` parsing case-insensitive, matching existing
      `#+title:` behavior.
- [x] Review property drawer parsing for case handling:
  - [x] `:PROPERTIES:` and `:END:` should be accepted case-insensitively.
  - [x] Property keys such as `:ID:`, `:CATEGORY:`, `:PROJECT:`,
        `:ROAM_ALIASES:`, and `:ROAM_REFS:` should follow org-mode casing
        expectations consistently.
- [x] Add parser tests covering:
  - [x] `#+BEGIN_SRC` / `#+END_SRC` content does not create links or tasks.
  - [x] `#+FILETAGS:` populates tags.
  - [x] Mixed-case drawer markers do not leak properties into body parsing.
- [x] Add one integration regression covering the previous false broken-link or
      false task behavior.
- [x] Run focused checks:
  - [x] `cargo test parser`
  - [x] `cargo test --test integration check`
  - [x] `cargo test --test integration task`

## Phase 3: Structured Output Contract

Goal: ensure `--output-format ndjson` never prints pretty multi-line JSON.

- [x] Split output format helpers in `src/output.rs`:
  - [x] `is_structured()` or equivalent for `json` and `ndjson`.
  - [x] `is_json()` for only `json`, if still useful.
  - [x] `print_json_line()` or equivalent for a single compact JSON object.
- [x] Update non-stream commands that currently treat `ndjson` as pretty JSON:
  - [x] `check`
  - [x] `info`
  - [x] `fix`
  - [x] `path`
  - [x] `new`
  - [x] `extract`
  - [x] `serve` startup output
  - [x] `init-config`
  - [x] startup and command error output
- [x] Decide and document the non-stream `ndjson` behavior:
  - [x] Prefer one compact JSON object on one line.
  - [x] Keep stream producers emitting one object per record.
- [x] Add integration tests that parse every line of non-stream `ndjson` output
      as exactly one JSON value where the command supports structured output.
- [x] Update docs if wording changes:
  - [x] `docs/json-output.md`
  - [x] `docs/pipelining.md`
  - [x] `skills/pkms-manager/references/pipelining.md`
- [x] Run focused checks:
  - [x] `cargo test output`
  - [x] `cargo test --test integration pipe`
  - [x] `cargo test --test integration all_commands`

## Phase 4: Task Sort And Group Validation

Goal: make task list and agenda validation consistent across PKMS-only and
source-neutral execution paths.

- [x] Extract shared task sort parsing and validation:
  - [x] Use one accepted-field list for all task list and agenda paths.
  - [x] Ensure empty sort fields are rejected consistently.
  - [x] Support only fields that each path can actually sort by, or document and
        implement missing fields for PKMS records.
- [x] Replace direct `split(',')` sort parsing in:
  - [x] `src/commands/task/todo.rs`
  - [x] `src/commands/task/agenda.rs`
- [x] Validate `task list --group` before rendering:
  - [x] Accept only `state`, `file`, and `priority`.
  - [x] Reject unknown group fields with a clear error.
- [x] Add integration tests covering:
  - [x] `task list --sort unknown` fails on the default PKMS path.
  - [x] `task agenda --sort unknown` fails on the default PKMS path.
  - [x] `task list --group unknown` fails.
  - [x] Valid sort and group fields still work.
- [x] Run focused checks:
  - [x] `cargo test task`
  - [x] `cargo test --test integration task`

## Phase 5: Config Strictness

Goal: reject typos in nested config tables instead of silently ignoring them.

- [x] Add `#[serde(deny_unknown_fields)]` to nested config structs where safe:
  - [x] `AgendaConfig`
  - [x] `TodoistConfig`
  - [x] `TaskConfig`
- [x] Add config tests for unknown fields in each nested table.
- [x] Confirm generated default config remains valid.
- [x] Run focused checks:
  - [x] `cargo test config`
  - [x] `cargo test --test integration config`

## Phase 6: Simplification Follow-Up

Goal: reduce the chance of future divergence between old PKMS task paths and
source-neutral task paths.

- [x] Inventory remaining duplicate task list and agenda responsibilities:
  - [x] Sort parsing.
  - [x] Limit handling.
  - [x] Date filtering.
  - [x] Priority filtering.
  - [x] Text, JSON, and NDJSON rendering.
- [x] Decide whether to migrate PKMS-only `TaskRecord` flows toward `TaskItem`
      earlier, or keep `TaskRecord` but share validation and rendering helpers.
- [x] Extract only abstractions with immediate duplication reduction; avoid a
      large rewrite.
- [x] Add regression tests before refactoring each behavior.

Decision: keep the PKMS-only `TaskRecord` flows for now because they preserve
canonical task ID and grouped table behavior directly. Share narrow helpers
instead: sort validation is now common across paths, and flat limit handling is
centralized. Date filtering, priority filtering, and full rendering migration
should wait for a deliberate `TaskRecord` to `TaskItem` migration with dedicated
coverage.

## Final Gate

Run the full feature matrix after the fixes are complete:

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --locked --all-targets -- -D warnings`
- [x] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [x] `cargo build --locked`
- [x] `cargo build --locked --features todoist`
- [x] `cargo build --locked --features web`
- [x] `cargo build --locked --features ssh`
- [x] `cargo build --locked --all-features`
- [x] `cargo test --locked`
- [x] `cargo test --locked --features todoist`
- [x] `cargo test --locked --features web`
- [x] `cargo test --locked --features ssh`
- [x] `cargo test --locked --features todoist,web,ssh`
- [x] `cargo run --locked -- --help`
- [x] `cargo run --locked --features todoist -- --help`
- [x] `cargo run --locked --features web -- --help`
- [x] `cargo run --locked --features ssh -- check --help`
- [x] `cargo build --locked --release`

## Notes

- [x] Keep each phase independently reviewable.
- [x] Update user docs and `skills/pkms-manager/` references when CLI behavior
      or output contracts change.
- [x] Do not change canonical task ID behavior while fixing task validation.
- [x] Keep `pkms serve` foreground-only and free of persistent derived state.
