# Configuration

`pkms` reads `~/.config/pkms.toml` for persistent settings.

```toml
db_root = "/home/user/Documents/org"
new_notes_dir = "roam"
ignore_patterns = [".attach", "*.bak"]

[tasks]
inbox = "Inbox"

[agenda]
open_todo_states = ["TODO", "WAITING", "IN-PROGRESS"]
closed_todo_states = ["DONE"]
```

## Database Root Resolution

The database root is resolved once at startup in this order:

1. `--db PATH`
2. `PKMS_DB_ROOT`
3. `db_root` from `~/.config/pkms.toml`

If none is available, the command exits with setup instructions.

## New Notes Directory

`new_notes_dir` controls where `pkms new --create` writes files. Relative paths
are resolved under `db_root`; absolute paths are used as written.

## Ignore Patterns

`ignore_patterns` are glob-style patterns skipped during recursive `.org` file
discovery. Use them for attachment directories, backups, generated files, and
other files that should not enter the graph.

## Task Inbox

`[tasks].inbox` configures the PKMS inbox note used by `pkms task inbox` and
default `pkms task add`. The value can be a note title, UUID, absolute path, or
path relative to `db_root`. Set it to `daily` to use today's daily note under a
top-level `* Inbox` heading.

## Agenda States

`open_todo_states` define headings treated as active tasks by `task list`.
`closed_todo_states` define completed task states. These state lists also affect
canonical task ID ordering.
