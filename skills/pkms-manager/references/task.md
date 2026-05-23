# pkms task

Task-first namespace for local PKMS tasks.

```bash
pkms task list
pkms task agenda
pkms task agenda --today
pkms task agenda --week
pkms task agenda --overdue
pkms task today
pkms task overdue
pkms task upcoming --days 7
pkms task show p5
pkms task open p5
pkms task state p5 WAITING
pkms task done p5
pkms task done p5 --dry-run
```

PKMS task IDs can be written as bare canonical IDs, `p<ID>`, or `pkms:<ID>`.
Top-level compatibility commands use bare numeric IDs.

`task state` changes only the TODO keyword. Valid states come from configured
`open_todo_states` and `closed_todo_states`. State input is case-insensitive,
but the file is written with the canonical config spelling.

`task done` is shorthand for the first configured closed state, defaulting to
`DONE` if none is configured.

Todoist read support is available only in builds made with `--features todoist`:

```bash
pkms task list source:todoist
pkms task list source:all
pkms task agenda --today source:todoist
pkms task agenda --week source:all
pkms task today source:all
pkms task overdue source:todoist
pkms task upcoming --days 7 source:all
pkms task inbox
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task done todoist:<remote-id>
```

Use stable `todoist:<remote-id>` IDs. Do not invent view-local Todoist IDs.
Never print the Todoist token. It is read from `TODOIST_API_TOKEN` by default,
or from `[todoist].token` in config when the environment variable is unset.

For Todoist-backed agenda views, `task agenda --today source:todoist` uses the
Todoist `today` filter, `--overdue` uses `overdue`, `--week` uses `next 7 days`,
and `--upcoming` uses `due after: today`. `todoist.filter:<query>` overrides
those generated agenda filters when the assistant needs custom Todoist syntax.

Prefer stable shortcuts for common assistant requests: `task today`,
`task overdue`, `task upcoming --days N`, and `task inbox`. The first three
accept `source:pkms`, `source:todoist`, or `source:all`; `task inbox` defaults
to Todoist and uses the Todoist `#Inbox` filter because PKMS has no local inbox
convention yet.

Use `--dry-run` before completing Todoist tasks when operating on a real token:

```bash
pkms task done todoist:<remote-id> --dry-run
```
