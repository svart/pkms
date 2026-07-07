# Configuration

`pkms` reads persistent settings from `~/.config/pkms.toml`. Unknown TOML
fields are rejected, so keep only the keys documented here. Command-line flags
and environment variables override config values only where noted.

## Complete Config File Example

This example shows every accepted TOML key. The column defaults have two
mutually exclusive forms: either a top-level `columns = [...]` list, or the
source/view-specific `[columns.*]` sections shown below.

```toml
db_root = "/home/user/Documents/org"
new_notes_dir = "roam"
daily_notes_dir = "roam/daily"
ignore_patterns = [".attach", "*.bak"]

# Alternative global task table default:
# columns = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading"]

[columns.pkms]
tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]

[columns.todoist]
tasks = ["Id", "State", "Prio", "Tags", "Project", "Heading"]
agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Heading"]

[tasks]
inbox = "Inbox"

[agenda]
open_todo_states = ["TODO", "WAITING", "IN-PROGRESS"]
closed_todo_states = ["DONE"]

[todoist]
enabled = false
token_env = "TODOIST_API_TOKEN"
token = "..." # optional; prefer an environment variable
default_filter = "today | overdue"

[rag]
rag_db = ".data/pkms-rag.sqlite3"
embedding_model = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"
fastembed_model_dir = "models/paraphrase-multilingual-MiniLM-L12-v2"
```

Generate a starter file with `pkms init-config`, or write a `db_root` value at
generation time with `pkms init-config --db PATH`.

## Database And Note Paths

These options identify the org-roam database and the directories used for new
notes and daily notes. Relative directory values are resolved under `db_root`.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| `db_root` | `PKMS_DB_ROOT` | global `--db PATH` | Resolution order is `--db`, then `PKMS_DB_ROOT`, then `db_root`. One of them is required. |
| `new_notes_dir` | - | - | Directory for `pkms new --create`. Defaults to `roam`. |
| `daily_notes_dir` | - | - | Directory for daily notes when `[tasks].inbox = "daily"`. Defaults to `new_notes_dir`. |
| `ignore_patterns` | - | - | Glob-style patterns skipped during recursive `.org` discovery. |

## Tasks, Agenda, And Columns

These options control local task state handling, the default PKMS inbox, and
task table columns. Available task columns are `Id`, `Date`, `State`, `Type`,
`Prio`, `Tags`, `Project`, `Note`, and `Heading`.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| `columns` | - | `pkms task list --columns COLS`, `pkms task agenda --columns COLS`, `pkms task inbox --columns COLS` | Global default task table columns. CLI values replace the configured set, or use `+Column`/`-Column` adjustments. |
| `[columns.pkms].tasks` | - | `pkms task list --columns COLS`, `pkms task inbox --columns COLS` | Default columns for local PKMS task lists and inbox views. |
| `[columns.pkms].agenda` | - | `pkms task agenda --columns COLS` | Default columns for local PKMS agenda-style views. |
| `[columns.todoist].tasks` | - | `pkms task list --columns COLS`, `pkms task inbox --columns COLS` | Default columns for Todoist task lists and inbox views. |
| `[columns.todoist].agenda` | - | `pkms task agenda --columns COLS` | Default columns for Todoist agenda-style views. For `source:all`, source defaults must match or `--columns` must be passed. |
| `[tasks].inbox` | - | - | Inbox note for `pkms task inbox` and default `pkms task add`. Use `daily` to append under today's daily note `* Inbox` heading. |
| `[agenda].open_todo_states` | - | - | States treated as active tasks by task commands. Defaults to `["TODO"]`. |
| `[agenda].closed_todo_states` | - | - | States treated as completed tasks. Defaults to `["DONE"]`. These state lists affect canonical task ID ordering. |

## Todoist

Todoist support requires a binary built with `--features todoist`. Environment
tokens are preferred over storing secrets in the config file.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| `[todoist].enabled` | - | - | Parsed as part of Todoist config. Current Todoist commands are gated by feature build and token availability. |
| `[todoist].token_env` | - | - | Name of the environment variable used for the Todoist token. Defaults to `TODOIST_API_TOKEN`. |
| `[todoist].token` | value named by `[todoist].token_env`, default `TODOIST_API_TOKEN` | - | Token fallback when the environment variable is unset or empty. Keep config files containing this value private. |
| `[todoist].default_filter` | - | - | Default Todoist filter used for list requests when no task filter `todoist.filter:<query>` is supplied. |
| - | `PKMS_TODOIST_API_BASE_URL` | - | Overrides the Todoist API base URL, mainly for tests and mock servers. Defaults to `https://api.todoist.com/api/v1`. |

`pkms info` does not print Todoist token values.

## RAG Retrieval

RAG options require a binary built with `--features rag`. Relative `[rag]` paths
are resolved under `db_root`; environment path overrides are used as written.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| `[rag].rag_db` | `PKMS_RAG_DB` | `pkms rag <cmd> --rag-db PATH` | SQLite index path. Resolution order is CLI, env, config, then `.data/pkms-rag.sqlite3`. |
| `[rag].embedding_model` | `PKMS_RAG_EMBEDDING_MODEL` | - | FastEmbed model name. The environment variable overrides the config value. |
| `[rag].fastembed_model_dir` | `PKMS_RAG_FASTEMBED_MODEL_DIR` | - | Local FastEmbed model directory. The environment variable overrides the config value. |
| - | `PKMS_RAG_EMBEDDING_PROVIDER` | - | Embedding provider: `fastembed` by default, or `hash` for deterministic local tests and fixtures. |
| - | - | `pkms rag index --embedding-batch-size N` | FastEmbed batch size for one index run. Defaults to `256`. |
| - | - | `pkms rag index --embedding-max-body-chars N` | Maximum body characters included in embedding text for one index run. Defaults to `8000`. |
| - | - | `pkms rag serve --notes-root PATH` | One-run org notes root override for the foreground RAG server. |
| - | - | `pkms rag serve --index-source PATH` | One-run retrieval NDJSON source override for the foreground RAG server. |
| - | - | `pkms rag serve --host HOST` | RAG HTTP bind host. Defaults to `127.0.0.1`. |
| - | - | `pkms rag serve --port PORT` | RAG HTTP bind port. Defaults to `7337`. |

`pkms rag index` is text-only, rejects `--output-format`, and uses the resolved
`db_root` as its notes source. `pkms rag serve` also uses `db_root` unless
`--notes-root` or `--index-source` is passed for that foreground server run.

## Diagnostics And Output

Diagnostics are disabled by default and are written to stderr, so command stdout
remains parseable for text, JSON, and NDJSON consumers.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| - | `PKMS_LOG` | - | Enables internal logs. `1` and `true` mean `debug`; `0`, `false`, and an empty value disable logs. Also accepts `tracing-subscriber` env-filter directives such as `pkms::config=debug`. |
| - | `PKMS_LOG_FORMAT` | - | Set to `json` for JSON log records. Any other value uses text logs. |
| - | `PKMS_LOG_HTTP` | - | Enables Todoist HTTP metadata logs without logging tokens, request bodies, task content, or descriptions. |
| - | `COLUMNS` | - | Overrides detected terminal width for adaptive task table layout. |
| - | `TERM`, `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` | - | Standard terminal color controls used for text styling. |
| - | - | global `--output-format FMT` | Selects stdout format: `text`, `json`, or `ndjson` where supported. |

Examples:

```bash
PKMS_LOG=debug pkms --db ~/Documents/org task agenda
PKMS_LOG=pkms::config=debug pkms info
PKMS_LOG_FORMAT=json PKMS_LOG=debug pkms --output-format json info
PKMS_LOG_HTTP=1 pkms task todoist:<remote-id> done
```

## SSH File-Link Checks

SSH file-link checks require a binary built with `--features ssh` and run only
when requested. There is no `[ssh]` configuration section.

| Configuration option | Environment variable | CLI flag | Notes |
|---|---|---|---|
| - | - | `pkms check --remote-file-links` | Enables remote SSH `file:` link checks for one run. |
| - | `USER`, `LOGNAME` | - | Default SSH username when a remote file link omits the user. `USER` is tried first. |

SSH checks use strict `~/.ssh/known_hosts` verification and non-interactive
public-key authentication. Standard passwordless identity files under `~/.ssh/`
and the SSH agent are tried automatically. Password, passphrase, and
keyboard-interactive prompts are never attempted.
