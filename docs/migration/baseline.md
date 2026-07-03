# Crate Split Baseline

Date: 2026-07-03
Branch: `migration`
Baseline commit: `e36ca71`

This file records the pre-split behavior baseline for the crate split migration.
It is intentionally documentation-only. No production code changed in this
baseline slice.

## Verification

The current full-feature gate passed before any extraction work:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-features
```

Observed test status:

- Unit tests: 300 passed.
- Integration tests: 330 passed.
- Ignored tests: 1 live SSH integration test, ignored unless live SSH test env vars are set.
- Doc tests: 0 tests.

The first `cargo test --all-features` attempt exceeded the 120 second tool
timeout while tests were still passing. The rerun with a longer timeout passed.

## Command Surface To Preserve

Top-level help captured with `cargo run --all-features -- --help`:

```text
org-roam PKMS navigation and validation tool

Usage: pkms [OPTIONS] <COMMAND>

Commands:
  check        Verify health of the entire org-roam database
  validate     Validate health of a specific note
  stats        Comprehensive database statistics
  orphans      List orphan notes (no incoming or outgoing links)
  resolve      Fast UUID/title resolution without full graph load
  fix          Repair broken UUID links or misplaced attachments
  suggest      Suggest related notes by multi-factor scoring (takes UUID only)
  new          Generate a filename and UUID for a new note
  extract      Extract a heading subtree into a new note
  get          Retrieve a note with its neighbors at specified depth
  query        Fuzzy search across note titles and content. Match sources: title, alias, ref, tag, content
  info         Show current pkms configuration
  init-config  Generate default config file at ~/.config/pkms.toml
  task         List, inspect, and update tasks across configured sources
  path         Find shortest path between two notes
  serve        Serve one rendered note and linked notes over local HTTP
  help         Print this message or the help of the given subcommand(s)

Options:
      --db <PATH>            Path to org-roam database root (overrides config)
      --output-format <FMT>  Output format [possible values: text, json, ndjson]
  -h, --help                 Print help
  -V, --version              Print version
```

Task help captured with `cargo run --all-features -- task --help`:

```text
List, inspect, and update tasks across configured sources

Usage: pkms task [OPTIONS] <COMMAND>

Commands:
  list    List tasks
  agenda  Show scheduled and deadline tasks
  inbox   Show inbox tasks
  add     Add a task
  help    Print this message or the help of the given subcommand(s)

Options:
      --db <PATH>            Path to org-roam database root (overrides config)
      --output-format <FMT>  Output format [possible values: text, json, ndjson]
  -h, --help                 Print help

ID-first actions:
  pkms task <ID> show
  pkms task <ID> open [--editor <COMMAND>] [--line <LINE>]
  pkms task <ID> state <STATE> [--dry-run]
  pkms task <ID> done [--dry-run]
  pkms task <ID> postpone --to <DATE>
  pkms task <ID> mod <MODIFIER>...
  pkms task <ID> mod dep:<PARENT-ID>

Task modifiers apply to `task add` and `task <ID> mod`.
  title:<text>              Task title; non-modifier words are task text for add only
  state:<state>             PKMS TODO state from configured agenda states
  tag:<label>, tags:<a,b>   Labels/tags; repeat or comma-separate
  schedule:<date>, sch:<date>, due:<date>
  deadline:<date>, dead:<date>, dl:<date>
  project:<name-or-id>, proj:<name-or-id>, prio:A|B|C, desc:<text>, source:pkms|todoist
  note:<uuid-title-or-path> PKMS add only; choose the note to append into
  dep:<task-id>, depend:<task-id> PKMS add/mod; add as child or move under parent
```

## Existing Output Contract Coverage

Representative structured output is covered by the current integration suite and
should remain green after each extraction slice.

- `tests/integration/all_commands.rs::test_all_commands_json` checks JSON object output for `check`, `stats`, `orphans`, `resolve`, `query`, `path`, `validate`, `get`, `suggest`, `new`, `fix`, `task agenda`, `task list`, `stats --todos`, `check --filetags`, and `task p1 show`.
- `tests/integration/all_commands.rs::test_non_stream_commands_ndjson_emit_single_json_line` checks single-object NDJSON for representative non-stream commands.
- `tests/integration/snapshot.rs` keeps human snapshots for `path` and `resolve` under `tests/integration/snapshots/`.
- `tests/integration/task.rs` covers task help, list, agenda, inbox, add, ID-first show/open/state/done/postpone/mod actions, Todoist feature behavior, canonical PKMS task IDs, and task mutation outputs.
- Command-specific integration files under `tests/integration/` cover text, JSON, NDJSON, error, and pipeline behavior for each command area.
