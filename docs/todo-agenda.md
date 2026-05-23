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
pkms task list source:pkms
```

`task list` and `task agenda` use source-neutral text columns:

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
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --project Inbox "Buy milk tomorrow"
pkms task done todoist:<remote-id>
pkms task done todoist:<remote-id> --dry-run
```

Todoist tasks are fetched only when the source set includes Todoist. Local
commands such as `pkms task list source:pkms` do not make Todoist requests.
For Todoist agenda views, `task agenda` translates high-level agenda flags to
Todoist server-side filters:

| Flag | Todoist filter |
|------|----------------|
| `--today` | `today` |
| `--overdue` | `overdue` |
| `--week` | `next 7 days` |
| `--upcoming` | `due after: today` |

An explicit `todoist.filter:<query>` takes precedence over agenda flags and is
the escape hatch for custom Todoist filter syntax.

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
