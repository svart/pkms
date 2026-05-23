# TODO and Agenda

`todo`, `agenda`, `show`, `open`, and the local `task` namespace share one
canonical task ID space. The ID shown by `todo` is the same ID used by
`agenda`, `show <ID>`, `open <ID>`, `task show p<ID>`, and `task open p<ID>`.

IDs are assigned globally using a deterministic sort:

1. Open task states first.
2. Closed task states after open tasks.
3. Within each group, by priority, path, and line number.

IDs remain stable while the underlying files do not change. Filtered views may
show non-contiguous IDs because excluded items still keep their global IDs.

The `task` namespace accepts these PKMS task ID forms:

```text
12
p12
pkms:12
```

Top-level compatibility commands keep accepting bare numeric IDs.

## TODO

```bash
pkms todo
pkms todo --columns Id,Date,State,Type,Prio,Tags,Note,Heading
pkms todo --group state
pkms todo --sort file
pkms todo --state "TODO"
pkms todo --state "!DONE"
pkms todo --tags "agenda,project"
pkms todo --type "SCHED"
pkms todo --prio A
pkms todo --after 2026-01-01
pkms todo --before 2026-01-31
pkms todo --scope <uuid-or-title>
pkms todo --line-sep
```

`todo` lists headings in configured open TODO states unless filters select other
states.

## Agenda

```bash
pkms agenda
pkms agenda --today
pkms agenda --week
pkms agenda --overdue
pkms agenda --upcoming
pkms agenda --date 2026-01-01
pkms agenda --columns Id,Date,State,Type,Prio,Tags,Note,Heading
pkms agenda --state "TODO"
pkms agenda --tags "!device,agenda"
pkms agenda --type "SCHED,DEADL"
pkms agenda --prio A
pkms agenda --sort "date,priority"
```

`agenda` shows headings with `SCHEDULED` or `DEADLINE` timestamps and can focus
on today, the current week, overdue items, upcoming items, or a specific date.

## Filters

Comma-separated filters use AND logic. Prefix a value with `!` for negation.

```bash
pkms todo --state "TODO"
pkms todo --state "!DONE,!CANCELLED"
pkms agenda --tags "agenda,project"
pkms agenda --tags "!private"
pkms agenda --type "SCHED,DEADL"
```

## Columns

Available table columns:

```text
Id,Date,State,Type,Prio,Tags,Note,Heading
```

Use `--line-sep` for row separator lines in text output.

## Show and Open

Inspect a task by canonical ID:

```bash
pkms show 5
pkms task show p5
pkms task show pkms:5
```

Open a task at its source heading:

```bash
pkms open 5
pkms task open p5
```

By default, `open` runs `emacsclient -n`. Override it with `--editor` or open a
specific line with `--line`.

```bash
pkms open <uuid-or-title> --editor "emacsclient -n"
pkms open <uuid-or-title> --line 42
```

Use `show --uuid <target>` when a numeric-looking target should be treated as a
note target rather than a canonical task ID.

## Task Namespace

The `task` namespace is the newer task-oriented command surface. Local PKMS
commands are available now:

```bash
pkms task list
pkms task agenda
pkms task agenda --today
pkms task agenda --week
pkms task agenda --overdue
pkms task today
pkms task overdue
pkms task upcoming --days 7
pkms task list source:pkms
```

`task list`, `task agenda`, and task shortcut commands use source-neutral text
columns:

```text
Id,Source,Date,State,Prio,Tags,Project,Task
```

JSON and NDJSON include source-neutral fields such as `source`, `source_id`,
`display_id`, `status`, `state`, `note_title`, `note_uuid`, `path`, and
`line_number`.

When built with `--features todoist`, Todoist read commands are available:

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
pkms task add --source todoist --project Inbox "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --label phone --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task done todoist:<remote-id> --dry-run
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule todoist:<remote-id> --due 2026-05-24
pkms task schedule todoist:<remote-id> --due none
pkms task update todoist:<remote-id> --title "Call Alice" --project Work --priority A
pkms task delete todoist:<remote-id> --dry-run
pkms task reopen todoist:<remote-id>
```

Todoist tasks are fetched only when the source set includes Todoist. Local
commands such as `pkms task list source:pkms` do not make Todoist requests.
For Todoist agenda views, `task agenda` translates high-level agenda flags to
Todoist server-side filters:

| Flag | Todoist filter |
|------|----------------|
| none | `!no date` |
| `--today` | `today` |
| `--overdue` | `overdue` |
| `--week` | `next 7 days` |
| `--upcoming` | `due after: today` |

Without a date flag, Todoist-backed `task agenda` shows only scheduled tasks.
An explicit `todoist.filter:<query>` takes precedence over the default agenda
filter and date flags, and is the escape hatch for custom Todoist filter syntax.

Use the stable shortcut commands for common assistant workflows:

| Command | Semantics |
|---------|-----------|
| `task today` | Today's PKMS agenda tasks by default; accepts `source:todoist` or `source:all`. |
| `task overdue` | Overdue PKMS agenda tasks by default; accepts `source:todoist` or `source:all`. |
| `task upcoming --days N` | Upcoming tasks after today through the next `N` days; defaults to 7. |
| `task inbox` | Todoist Inbox tasks using the `#Inbox` filter. |

`task inbox` defaults to Todoist because PKMS does not yet define a local inbox
convention.

Use `task report` or `task plan` for assistant-facing summaries instead of
raw `list` or `agenda` output. Both commands build on source-neutral `TaskItem`
records and emit stable JSON with `total`, `sections`, `by_source`, and
`by_project`. Sections include `overdue`, `today_timed`, `today_untimed`,
`high_priority`, `waiting_or_blocked`, `inbox_or_no_date`, and `upcoming`.
`--source pkms|todoist|all` is accepted for report-style commands, and
`--upcoming-days N` controls the upcoming section window. With Todoist and
`--today`, the command uses a Todoist filter covering today, overdue, no-date,
and next-`N`-days tasks before grouping the returned items locally.

Use `task clarify --source todoist --output-format json` for inbox hygiene and
pre-planning review. It does not mutate tasks. JSON output returns tasks with
structured clarification reasons such as `no_date`, `no_project`, `no_context`,
`vague_title`, and `stale_inbox`. Text output is intentionally short for a quick
inbox review. The initial stale Inbox threshold is conservative and can be
overridden with `--stale-days N`.

Todoist task creation supports two modes. Positional text uses Todoist Quick Add
and lets Todoist parse natural language, labels, priorities, and projects:

```bash
pkms task add --source todoist "Buy milk tomorrow #Inbox @errand p1"
```

Structured creation uses Todoist API fields and is the safer mode for assistant
workflows:

```bash
pkms task add --source todoist \
  --title "Call Alice" \
  --due 2026-05-24 \
  --deadline 2026-05-30 \
  --project inbox \
  --label phone \
  --label migration \
  --priority B \
  --description "Discuss migration plan"
```

Structured `--due` and `--deadline` values must be `YYYY-MM-DD`. Priorities use
the source-neutral `A`, `B`, or `C` convention. Multiple `--label` flags are
allowed. `--project` accepts either a Todoist project id or an exact project
name. If a name matches multiple projects case-insensitively, `pkms` fails before
creating the task and asks for the project id.

Use `--note <uuid-or-title>` when a Todoist task should keep PKMS context.
`pkms` resolves the note and appends a durable marker to the Todoist description:
`pkms:id:<uuid>`. Existing descriptions are preserved and the marker is appended
after a blank line. Listing or showing Todoist tasks detects this marker and
populates `note_uuid` and `note_title` when the note exists locally. The marker
reveals a PKMS note UUID to Todoist; avoid `--note` for tasks where even that
identifier should not leave the local database.

Use `task projects source:todoist` and `task labels source:todoist` to inspect
available Todoist metadata. Todoist task output sets `project` to the display
name when metadata is available and keeps the raw id in `project_id`.

`task add --source todoist --output-format json` returns a wrapper with
`created: true` and the created source-neutral `TaskItem` under `item`, including
stable `display_id`, `source_id`, title, description, priority, dates, labels,
project, `project_id`, and URL when Todoist provides them. Text output stays concise but
includes the stable id and key planning fields.

Todoist mutation commands are source-specific and require stable
`todoist:<remote-id>` ids. `postpone` and `schedule` update the due date;
`schedule --due none` clears it. `update` can change title, project, priority,
labels, and description. `delete` supports `--dry-run`; without `--dry-run` it
calls Todoist delete. Changed-task JSON output returns `changed: true`, an
`action`, and the updated source-neutral `TaskItem`.

Set the token in the environment when possible. Environment variables take
precedence over config values and avoid storing the secret in a dotfile:

```bash
export TODOIST_API_TOKEN=...
```

Optional config:

```toml
[todoist]
enabled = false
token = "..." # optional; prefer TODOIST_API_TOKEN when practical
token_env = "TODOIST_API_TOKEN"
default_filter = "today | overdue"
```

If `token` is set in config, keep `~/.config/pkms.toml` private and do not check
it into git. `pkms info` does not print the token.

`todoist.filter:` uses Todoist's server-side filter endpoint. Pagination is
handled automatically.

Todoist writes are explicit. `task add --source todoist` uses Todoist Quick Add
semantics, including natural language dates, labels, priorities, and project
syntax. `--project NAME` appends `#NAME` to the Quick Add text.

`task done todoist:<remote-id>` calls Todoist's close endpoint. Use `--dry-run`
to print the planned completion without sending a Todoist request.

## State Changes

`task state` changes only the TODO keyword on the target heading. Valid states
come from `open_todo_states` and `closed_todo_states` in config.

```bash
pkms task state p5 WAITING
pkms task state pkms:5 done
pkms task done p5
pkms task done p5 --dry-run
```

State input is case-insensitive. The file is written with the canonical spelling
from config, so `done`, `Done`, and `DONE` all write `DONE` when `DONE` is the
configured state.

`task done` is shorthand for setting the first configured closed state. If no
closed state is configured, it defaults to `DONE`.
