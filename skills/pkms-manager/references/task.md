# pkms task

Task-first namespace for local PKMS tasks.

```bash
pkms task list
pkms task agenda
pkms task agenda --today
pkms task agenda --week
pkms task agenda --overdue
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
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task done todoist:<remote-id>
```

Use stable `todoist:<remote-id>` IDs. Do not invent view-local Todoist IDs.
Never print or store the Todoist token; it is read from `TODOIST_API_TOKEN` by
default.

Use `--dry-run` before completing Todoist tasks when operating on a real token:

```bash
pkms task done todoist:<remote-id> --dry-run
```
