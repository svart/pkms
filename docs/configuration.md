# Configuration

`pkms` reads `~/.config/pkms.toml` for persistent settings.

```toml
db_root = "/home/user/Documents/org"
new_notes_dir = "roam"
daily_notes_dir = "roam/daily"
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

## Diagnostic Logging

Runtime diagnostics are disabled by default. Set `PKMS_LOG` to enable structured
internal logs on stderr without changing command stdout:

```bash
PKMS_LOG=debug pkms --db ~/Documents/org task agenda
PKMS_LOG=pkms::config=debug pkms info
PKMS_LOG_FORMAT=json PKMS_LOG=debug pkms --output-format json info
```

`PKMS_LOG=1` and `PKMS_LOG=true` are aliases for `debug`. `PKMS_LOG` also
accepts `tracing-subscriber` env-filter directives, so module-specific targets
can be enabled without turning on all debug logs.

Todoist HTTP metadata can be enabled separately:

```bash
PKMS_LOG_HTTP=1 pkms task todoist:<remote-id> done
```

HTTP diagnostics include request method, Todoist API path, response status,
content length, pagination counts, and transport errors. They intentionally do
not log authorization tokens, request bodies, task content, descriptions, or
other Todoist payload fields.

## New Notes Directory

`new_notes_dir` controls where `pkms new --create` writes files. Relative paths
are resolved under `db_root`; absolute paths are used as written.

## Daily Notes Directory

`daily_notes_dir` controls where `pkms` creates daily notes when `[tasks].inbox`
is set to `daily`. Relative paths are resolved under `db_root`; absolute paths
are used as written. When unset, daily notes use `new_notes_dir` for backward
compatibility.

## Ignore Patterns

`ignore_patterns` are glob-style patterns skipped during recursive `.org` file
discovery. Use them for attachment directories, backups, generated files, and
other files that should not enter the graph.

## Task Inbox

`[tasks].inbox` configures the PKMS inbox note used by `pkms task inbox` and
default `pkms task add`. The value can be a note title, UUID, absolute path, or
path relative to `db_root`. Set it to `daily` to use today's daily note under a
top-level `* Inbox` heading.

## Todoist Integration

Todoist commands require a binary built with `--features todoist` and a token.
The token is read from `TODOIST_API_TOKEN` by default, or from the environment
variable named by `[todoist].token_env`. If that environment variable is unset
or empty, `[todoist].token` is used.

```toml
[todoist]
token_env = "TODOIST_API_TOKEN"
token = "..." # optional; prefer an environment variable
default_filter = "today | overdue"
```

`default_filter` is used for Todoist task list requests when no
`todoist.filter:<query>` filter is provided. `enabled` is accepted in the config
file for compatibility with generated configs, but current Todoist commands are
gated by the feature build and token availability.

Keep config files containing `[todoist].token` private. `pkms info` does not
print token values.

## Agenda States

`open_todo_states` define headings treated as active tasks by `task list`.
`closed_todo_states` define completed task states. These state lists also affect
canonical task ID ordering.

Default task table columns can be configured with the global top-level
`columns` array, or per source and view:

```toml
[columns.pkms]
tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]

[columns.todoist]
tasks = ["Id", "State", "Prio", "Tags", "Project", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Heading"]
```

Available task columns are `Id`, `Date`, `State`, `Type`, `Prio`, `Tags`,
`Project`, `Note`, and `Heading`. For `source:all`, source-specific defaults
must resolve to the same column set; otherwise pass `--columns` explicitly.
