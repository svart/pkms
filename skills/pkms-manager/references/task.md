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
pkms task report --today --source all --output-format json
pkms task plan --today --source all --output-format json
pkms task clarify --source todoist --output-format json
pkms task inbox
pkms task projects source:todoist
pkms task labels source:todoist
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --label phone --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule todoist:<remote-id> --due none
pkms task update todoist:<remote-id> --title "Call Alice" --project Work --priority A
pkms task delete todoist:<remote-id> --dry-run
pkms task reopen todoist:<remote-id>
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

Use `task report --today --source all --output-format json` or
`task plan --today --source all --output-format json` when an assistant needs a
daily summary or planning context. Prefer these over raw `task list` or
`task agenda` because the JSON is pre-grouped into `overdue`, `today_timed`,
`today_untimed`, `high_priority`, `waiting_or_blocked`, `inbox_or_no_date`,
`upcoming`, `by_source`, and `by_project`. With Todoist and `--today`, report
commands use a Todoist filter for today, overdue, no-date, and upcoming tasks
before grouping the returned items locally.

Use `task clarify --source todoist --output-format json` before planning when
the assistant needs to identify tasks that need date, project, scope, or context.
It is read-only and returns per-task reason codes: `no_date`, `no_project`,
`no_context`, `vague_title`, and `stale_inbox`. Use text output for a short
inbox review and JSON output for automated assistant decisions.

Use positional text with `task add --source todoist` for Todoist Quick Add
natural-language parsing. Use structured creation for deterministic assistant
tasks:

```bash
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --deadline 2026-05-30 --project inbox --label phone --priority B --description "Discuss migration plan"
```

Structured due and deadline values must be `YYYY-MM-DD`. Priority must be `A`,
`B`, or `C`. Repeat `--label` for multiple labels. `--project` accepts either a
Todoist project id or an exact project name. If a project name is duplicated
case-insensitively, use the project id.

Use `--note <uuid-or-title>` when a Todoist task needs durable PKMS context.
The command appends `pkms:id:<uuid>` to the Todoist description without removing
user-authored description text. Later `task list source:todoist` and
`task show todoist:<remote-id>` populate `note_uuid` and `note_title` from that
marker. Treat this as a privacy boundary: the PKMS UUID is stored in Todoist.

Use `task projects source:todoist` and `task labels source:todoist` when you
need available Todoist metadata. Todoist task output uses a human-readable
`project` when metadata is available and keeps the raw id in `project_id`.

JSON output from `task add --source todoist` is a creation wrapper with
`created: true` and the created source-neutral `TaskItem` in `item`. Use
`item.display_id` for confirmation to the user and `item.source_id` for the raw
Todoist id.

Use `--dry-run` before completing Todoist tasks when operating on a real token:

```bash
pkms task done todoist:<remote-id> --dry-run
```

Use stable `todoist:<remote-id>` ids for Todoist mutations. `task postpone`
accepts `--to tomorrow` or `--to YYYY-MM-DD`; `task schedule` accepts
`--due tomorrow`, `--due YYYY-MM-DD`, or `--due none`; `task update` can change
title, project, priority, labels, and description. Always use
`task delete ... --dry-run` before deleting real Todoist tasks. `task reopen`
reopens completed Todoist tasks.
