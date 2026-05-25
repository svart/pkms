# Task Command Replacement Plan

## Goal

Make `pkms task list` and `pkms task agenda` strict supersets of `pkms todo`
and `pkms agenda`, then deprecate and eventually remove the top-level commands
without losing behavior, scriptability, or documented workflows.

The main design decision is that the newer `task` commands should keep the
richer source-neutral model, but they need a compatibility path for every
observable legacy behavior: filtering, grouping, sorting, date semantics, stdin
pipelines, text layout, JSON/NDJSON fields, and documentation.

## Target Contract

| Legacy command | Replacement command | Required parity |
|---|---|---|
| `pkms todo` | `pkms task list` | Same default output, same filters, same grouping, same scope/stdin support, same sort semantics, same JSON/NDJSON data or documented compatibility mode. |
| `pkms agenda` | `pkms task agenda` | Same default output, same date shortcuts, same filters, same sort semantics, same daily-file behavior, same JSON/NDJSON data or documented compatibility mode. |

## Phase 1: Define Compatibility Spec

### Task 1: Write a compatibility spec

Document the exact compatibility contract before implementation.

Acceptance criteria:

- Every `todo` option has a documented `task list` equivalent.
- Every `agenda` option has a documented `task agenda` equivalent.
- The output schema strategy is explicit: either source-neutral task JSON is the
  only future schema, or an exact legacy compatibility mode is added.
- The deprecation policy is explicit: warn first, remove later.

Likely files:

- `docs/todo-agenda.md`
- `docs/commands.md`
- Optional new migration document under `docs/`

Recommendation: do not change the default `task` JSON back to the legacy shape.
The source-neutral schema is better long term. If existing scripts need exact
legacy fields, add a compatibility mode instead.

## Phase 2: Unify Task Data Model

### Task 2: Extend `TaskItem` to cover legacy fields

The source-neutral `TaskItem` must be able to reproduce every field or behavior
currently provided by `TodoItem` and `AgendaItem`.

Current gaps include daily-file dates, `has_agenda_tag`, `heading_level`, and
legacy naming concepts such as `todo_state` and `heading_title`.

Acceptance criteria:

- PKMS `TaskItem` carries enough information to reproduce `TodoItem`.
- PKMS `TaskItem` carries enough information to reproduce `AgendaItem`.
- Daily-file agenda entries participate in filtering, sorting, and display
  exactly like legacy agenda.
- No legacy-only collection path is required for correctness.

Likely files:

- `src/tasks/model.rs`
- `src/tasks/pkms.rs`
- `src/commands/task.rs`

### Task 3: Centralize PKMS task collection

Move PKMS task collection behind shared helpers that can serve `task`, `todo`,
and `agenda`.

Acceptance criteria:

- `task list` does not need to delegate to `todo::run` for default parity.
- `task agenda` does not need to delegate to `agenda::run` for default parity.
- Legacy `todo` and `agenda` can call the same task-layer helpers or become
  thin adapters.
- Canonical task ID assignment remains unchanged and shared.

Likely files:

- `src/commands/task_index.rs`
- `src/commands/task.rs`
- `src/commands/todo.rs`
- `src/commands/agenda.rs`

## Phase 3: Close `task list` Gaps

### Task 4: Add `task list --group`

Implement grouping parity with `todo --group`.

Acceptance criteria:

- `pkms task list --group state` works.
- `pkms task list --group file` works.
- `pkms task list --group priority` works.
- Text grouping matches `todo --group`.
- JSON grouping exposes equivalent `group_field` and `groups`.
- NDJSON includes `group`, matching legacy behavior.
- `--limit` behavior matches legacy: the limit applies per group.

Likely tests:

- Mirror existing `todo --group` coverage from `tests/integration/todo.rs`.

### Task 5: Add `task list --from-stdin`

Restore pipeline-consumer parity with `todo --from-stdin`.

Acceptance criteria:

- `pkms resolve --tags project --output-format ndjson | pkms task list --from-stdin` works.
- The command reads UUIDs from NDJSON using the same behavior as `todo --from-stdin`.
- It composes with filters, sorting, grouping, columns, and limit.
- `docs/pipelining.md` uses `task list --from-stdin` in examples.

Likely files:

- `src/cli.rs`
- `src/main.rs`
- `src/commands/task.rs`
- `docs/pipelining.md`

### Task 6: Complete `task list` sort parity

Acceptance criteria:

- Supports all legacy sort fields: `priority`, `state`, `file`, `date`.
- Keeps richer task fields: `source`, `task`, and `title`.
- Invalid sort fields fail clearly instead of silently becoming no-ops.
- Multi-field sort remains supported.

### Task 7: Confirm `task list` filter parity

Acceptance criteria:

- `todo --state X` maps to `task list state:X`.
- `todo --tags X` maps to `task list tags:X`.
- `todo --type X` maps to `task list type:X`.
- `todo --prio A` maps to `task list prio:A`.
- `todo --prio ""` maps to `task list prio:none`.
- `todo --scope A --scope B` maps to multiple `scope:` filters.
- `todo --after` and `todo --before` match task-layer datetime behavior.
- Daily-file tasks are included or excluded exactly as legacy `todo` handles
  them.

## Phase 4: Close `task agenda` Gaps

### Task 8: Match agenda date semantics exactly

Acceptance criteria:

- `task agenda` matches `agenda`.
- `task agenda today` and `task agenda date:today` match `agenda --today`.
- `task agenda week` and `task agenda date:week` match `agenda --week`.
- `task agenda overdue` and `task agenda overdue` filter syntax match
  `agenda --overdue`.
- Upcoming has an unbounded legacy-equivalent form.

Current risk: `task agenda upcoming --days 7` is richer but not equivalent to
legacy `agenda --upcoming`, which is unbounded. Add one of these:

- `pkms task agenda upcoming --all`
- `pkms task agenda upcoming --days unlimited`
- documented and tested `pkms task agenda date:upcoming` as the exact legacy
  equivalent

### Task 9: Preserve daily-file agenda behavior

Acceptance criteria:

- Daily note items appear in agenda exactly as legacy agenda includes them.
- `date:today`, `date:week`, `date:YYYY-MM-DD`, `upcoming`, and sorting all
  account for daily-file dates.
- Text output includes the same date display behavior as legacy where relevant.
- JSON/NDJSON output does not lose daily-file date information.

### Task 10: Complete agenda sort parity

Acceptance criteria:

- Supports legacy sort fields: `date`, `priority`, `scheduled`, `deadline`,
  and `file`.
- Keeps richer task fields: `source`, `state`, `task`, `title`, and `project`.
- Invalid sort fields fail clearly instead of silently becoming no-ops.
- Multi-field sort remains supported.

### Task 11: Confirm agenda filter parity

Acceptance criteria:

- `agenda --state` equivalent exists.
- `agenda --tags` equivalent exists.
- `agenda --type` equivalent exists.
- `agenda --prio` equivalent exists.
- `agenda --date` equivalent exists.
- `agenda --today`, `--week`, `--overdue`, and `--upcoming` equivalents are
  covered by integration tests.

## Phase 5: Output Compatibility

### Task 12: Decide and implement JSON/NDJSON migration strategy

Recommended contract:

- `task` default JSON remains source-neutral.
- Add `--output-schema legacy` or `--compat-output legacy` only for PKMS-only
  `task list` and `task agenda`, if exact legacy script compatibility is needed.
- Legacy top-level commands keep the old schema until removal.
- Migration docs show field mapping.

Acceptance criteria:

- Users can reproduce old `todo` JSON from `task list` if exact compatibility is
  required.
- Users can reproduce old `agenda` JSON from `task agenda` if exact
  compatibility is required.
- Source-neutral JSON remains available and documented as the preferred future
  schema.
- NDJSON behavior is documented for both the source-neutral and compatibility
  modes.

### Task 13: Add field mapping docs

Document the mapping from legacy fields to source-neutral fields.

Important mappings:

- `todo_state` -> `state`
- `heading_title` -> `title`
- legacy note `title` -> `note_title`
- `uuid` -> `note_uuid`
- `id` -> `source_id` or the bare text ID in PKMS-only views
- `scheduled_date` -> `scheduled.date`
- `deadline_date` -> `deadline.date`
- `daily_file_date` -> a source-neutral daily-file date field, if retained

## Phase 6: CLI Ergonomics

### Task 14: Add compatibility aliases where useful

Acceptance criteria:

- `task list --state TODO` is either supported or rejected with a helpful
  message pointing to `state:TODO`.
- `task list --tags`, `--type`, `--prio`, `--scope`, `--after`, and `--before`
  are either supported or rejected with helpful migration messages.
- `task agenda --today`, `--week`, `--overdue`, `--upcoming`, `--date`,
  `--state`, `--tags`, `--type`, and `--prio` are either supported or rejected
  with helpful migration messages.
- Help output makes the replacement path obvious.

Recommendation: support aliases for low-risk flags where clap can do it cleanly.
This reduces migration friction and makes later removal less disruptive.

## Phase 7: Tests

### Task 15: Build parity integration tests

Acceptance criteria:

- For every existing `todo` integration test, add an equivalent `task list`
  test.
- For every existing `agenda` integration test, add an equivalent `task agenda`
  test.
- Include text, JSON, and NDJSON coverage.
- Include grouped output, scoped output, stdin pipeline, columns, line
  separators, limit, sort, filters, and date shortcuts.

Likely files:

- `tests/integration/todo.rs`
- `tests/integration/agenda.rs`
- `tests/integration/task.rs`
- `tests/integration/all_commands.rs`

### Task 16: Add regression tests for schema choices

Acceptance criteria:

- Source-neutral task JSON is stable.
- Legacy compatibility JSON, if added, is stable.
- NDJSON pipeline output remains parseable and useful.
- Todoist and mixed-source outputs remain source-neutral and are not forced into
  PKMS-only legacy shapes.

## Phase 8: Documentation and Skill Updates

### Task 17: Update user docs

Acceptance criteria:

- `docs/commands.md` presents `task list` and `task agenda` as the preferred
  commands.
- `docs/todo-agenda.md` includes a migration table.
- `docs/pipelining.md` replaces `todo --from-stdin` examples with
  `task list --from-stdin`.
- README examples use `task` commands.
- Todoist examples still show source-neutral task workflows.

### Task 18: Update agent workflow docs

Acceptance criteria:

- `skills/pkms-manager/SKILL.md` prefers `task list` and `task agenda`.
- Task references under `skills/pkms-manager/references/` are updated when task
  workflows change.
- References still document legacy commands while they exist, but mark them as
  compatibility commands once deprecation starts.

## Phase 9: Deprecation Path

### Task 19: Add advisory deprecation warnings after parity exists

Acceptance criteria:

- `pkms todo` prints a warning to stderr.
- `pkms agenda` prints a warning to stderr.
- Warning text gives the exact replacement command.
- JSON and NDJSON stdout remain machine-parseable; warnings must not pollute
  stdout.
- Tests assert warnings go to stderr only.

### Task 20: Keep adapters for one release cycle or more

Acceptance criteria:

- `todo` and `agenda` call the same implementation as `task`.
- No divergent task listing or agenda logic remains.
- Removal can later delete only CLI adapters, deprecated docs references, and
  tests for deprecated entrypoints.

### Task 21: Final removal

Acceptance criteria:

- Remove `Command::Todo` and `Command::Agenda`.
- Remove or rewrite legacy command modules if they are no longer used.
- Remove deprecation notices from active command docs.
- Keep migration notes in release notes or a historical migration document.
- Every old behavior remains reachable through `task list` or `task agenda`.

## Verification Gate

After implementation, run the repository verification sequence:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo clippy --features todoist -- -D warnings
cargo build
cargo build --features=embed
cargo build --features=todoist
cargo build --features=embed,todoist
cargo test
cargo test --features todoist
cargo test --test integration
cargo test --features todoist --test integration
cargo run -- --help
cargo run --features todoist -- --help
```

For changes that affect Todoist-specific behavior, also run focused Todoist
integration tests with `--features todoist` and the relevant test names.

## Recommended Order

1. Write the compatibility spec and migration table.
2. Extend the data model, especially daily-file date support.
3. Centralize PKMS task collection.
4. Add `task list --group`.
5. Add `task list --from-stdin`.
6. Complete sort and filter parity.
7. Complete agenda date and upcoming parity.
8. Decide and implement output compatibility.
9. Add full parity integration tests.
10. Update docs and skill references.
11. Add deprecation warnings.
12. Remove legacy entrypoints in a later release.

## Key Risks

| Risk | Impact | Mitigation |
|---|---|---|
| JSON/NDJSON schema changes break scripts | High | Keep legacy commands until migration is documented; add compatibility schema if needed. |
| Daily-file agenda behavior is lost in source-neutral `TaskItem` | High | Add explicit daily-file fields and parity tests before removing legacy code. |
| Sort fields silently differ | Medium | Validate sort fields and add parity tests. |
| Todoist source-neutral output regresses while adding PKMS compatibility | Medium | Keep Todoist tests under `--features todoist` in the verification gate. |
| `task` becomes cluttered with legacy flags | Medium | Prefer documented positional filters, but add aliases only where migration friction is high. |

