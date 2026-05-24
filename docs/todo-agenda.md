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
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task list source:pkms
```

By default, `task list` mirrors `todo` output and `task agenda` mirrors
`agenda` output for PKMS tasks. Explicit Todoist and mixed-source task views use
source-neutral text columns:

```text
Id,Date,State,Type,Prio,Tags,Project,Note,Heading
```

Text task views render IDs for the selected source set. In source-neutral views
with only one source selected, `Id` is the bare source id. With `source:all`,
PKMS ids are prefixed with `p` and Todoist ids are prefixed with `t`.

Source-neutral JSON and NDJSON include fields such as `source`, `source_id`,
`display_id`, `status`, `state`, `note_title`, `note_uuid`, `path`, and
`line_number`.

Item-producing task commands accept positional string filters after the
subcommand. These filters are separate from normal command flags such as
`--limit`, `--sort`, and `--columns`. Metadata commands, `task list projects`
and `task list tags`, accept only source filters.

| Filter | Syntax | Notes |
|--------|--------|-------|
| Source | `source:pkms`, `source:todoist`, `source:all` | Select local PKMS tasks, Todoist tasks, or both. Defaults to `source:pkms`. |
| Source alias | `src:pkms`, `src:todoist`, `src:all` | Short form of `source:`. |
| Todoist raw filter | `todoist.filter:<query>` | Uses Todoist's server-side filter endpoint. Requires `source:todoist` or `source:all`. |
| State | `state:TODO` | Matches TODO state case-insensitively. |
| State exclusion | `state:!DONE` | Excludes matching states. Comma-separated state filters use AND logic. |
| Tags | `tags:tag1,tag2` | Matches combined note filetags and heading tags for PKMS, and labels for Todoist. Tags are exact. |
| Tag alias | `tag:tag1,tag2` | Short form of `tags:`. |
| Tag exclusion | `tags:!tag1,tag2` | Excludes `tag1` and requires `tag2`. Comma-separated tag filters use AND logic. |
| Type | `type:SCHED`, `type:DEADL` | Matches scheduled or deadline tasks. `kind:` is an alias. |
| Type exclusion | `type:!SCHED` | Excludes tasks with a scheduled timestamp. |
| Priority | `prio:A`, `priority:A` | Matches priority `A`, `B`, or `C`; matching is case-insensitive. |
| No priority | `prio:none`, `priority:none` | Matches tasks without a priority. |
| Exact agenda date | `date:YYYY-MM-DD` | Matches scheduled or deadline dates on that day. |
| Today | `date:today` | Same date semantics as `agenda --today`. |
| Week | `date:week` | Same date semantics as `agenda --week`. |
| Overdue | `date:overdue`, `overdue` | Matches overdue tasks. |
| Upcoming | `date:upcoming`, `upcoming` | Matches non-overdue tasks after today. |
| After | `after:YYYY-MM-DD`, `after:YYYY-MM-DD HH:MM` | Same datetime semantics as `todo --after`. |
| Before | `before:YYYY-MM-DD`, `before:YYYY-MM-DD HH:MM` | Same datetime semantics as `todo --before`. |
| Scope | `scope:<target>` | Restricts PKMS tasks to a note title, UUID, or path, like `todo --scope`. |
| Project | `project:<name-or-id>` | Matches PKMS `PROJECT` properties and Todoist project names or ids. |
| Project exclusion | `project:!<name-or-id>` | Excludes matching projects. |

Examples:

```bash
pkms task list state:TODO tags:work,!blocked prio:A
pkms task agenda week type:SCHED project:Alpha
pkms task agenda source:all date:overdue tags:!waiting
pkms task agenda upcoming --days 14 source:all priority:B
pkms task list 'scope:Some Note Title' after:2026-05-01 before:"2026-05-25 18:00"
pkms task list source:todoist 'todoist.filter:today | overdue'
```

When built with `--features todoist`, Todoist read commands are available:

```bash
pkms task list source:todoist
pkms task list source:all
pkms task agenda today source:todoist
pkms task agenda week source:all
pkms task agenda today source:all
pkms task agenda overdue source:todoist
pkms task agenda upcoming --days 7 source:all
pkms task inbox
pkms task inbox source:todoist
pkms task list projects source:todoist
pkms task list tags source:todoist
pkms task list projects source:all
pkms task list tags source:all
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task show todoist:<remote-id>
pkms task add "Capture local task"
pkms task add --title "Call Alice" --due 2026-05-24 --deadline 2026-05-30 --label phone --priority B
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --project Inbox "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --label phone --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task done todoist:<remote-id> --dry-run
pkms task postpone p<id> --to 2026-06-01
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule p<id> --due 2026-05-24
pkms task schedule todoist:<remote-id> --due 2026-05-24
pkms task schedule todoist:<remote-id> --due none
pkms task deadline p<id> --deadline 2026-05-30
pkms task deadline todoist:<remote-id> --deadline none
```

Todoist tasks are fetched only when the source set includes Todoist. Local
commands such as `pkms task list source:pkms` do not make Todoist requests.
For Todoist agenda views, `task agenda` translates high-level agenda commands
and flags to Todoist server-side filters:

| Command or flag | Todoist filter |
|-----------------|----------------|
| none | `!no date` |
| `today` | `today` |
| `overdue` | `overdue` |
| `week` | `next 7 days` |
| `upcoming --days N` | `due after: today & next N days` |

Without an agenda shortcut, Todoist-backed `task agenda` shows only scheduled tasks.
An explicit `todoist.filter:<query>` takes precedence over the default agenda
filter and agenda command, and is the escape hatch for custom Todoist filter
syntax.

Use the stable agenda shortcut commands for common assistant workflows:

| Command | Semantics |
|---------|-----------|
| `task agenda today` | Today's PKMS agenda tasks by default; accepts `source:todoist` or `source:all`. |
| `task agenda overdue` | Overdue PKMS agenda tasks by default; accepts `source:todoist` or `source:all`. |
| `task agenda upcoming --days N` | Upcoming tasks after today through the next `N` days; defaults to 7. |
| `task inbox` | PKMS inbox-note tasks by default; accepts `source:todoist` or `source:all`. |

`task inbox` and default `task add` require a configured PKMS inbox note:

```toml
[tasks]
inbox = "Inbox"
```

The inbox value can be a note title, UUID, absolute path, or path relative to
`db_root`. Set it to `daily` to use today's daily note and place new tasks under
the `* Inbox` heading, creating the heading when needed. Without an inbox
configuration, PKMS inbox commands fail. `task inbox source:todoist` continues
to use Todoist's `#Inbox` filter.

PKMS task creation appends a TODO heading to that inbox note. It accepts
positional text or `--title`, `--due`, `--deadline`, `--label`, `--priority`,
and `--description`:

```bash
pkms task add "Capture local task"
pkms task add --title "Call Alice" --due 2026-05-24 --deadline 2026-05-30 --label phone --priority B
```

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

Use `task list projects` and `task list tags` to inspect task metadata. With
`source:pkms`, projects come from note-level or heading-level `PROJECT`
properties, and tags combine note `#+filetags` with heading tags as in `todo`
and `agenda`. With `source:todoist`, projects and tags come from Todoist
metadata. `source:all` combines both sources. Todoist task output sets `project`
to the display name when metadata is available and keeps the raw id in
`project_id`.

`task add --output-format json` returns a wrapper with
`created: true` and the created source-neutral `TaskItem` under `item`, including
stable `display_id`, `source_id`, title, description, priority, dates, labels,
project, `project_id`, and URL when Todoist provides them. Text output stays concise but
includes the stable id and key planning fields.

`task schedule` and `task deadline` work for PKMS and Todoist tasks. For PKMS,
they edit the heading planning line. For Todoist, they update `due_date` and
`deadline_date`. Use `--due none` or `--deadline none` to clear a date.
`task postpone` works only for recurring tasks. For PKMS, the task must have a
recurring `SCHEDULED` or `DEADLINE` timestamp and the repeater/warning syntax is
preserved. For Todoist, the Todoist due date must be recurring. Non-recurring
tasks fail instead of being silently rescheduled. Changed-task JSON output
returns `changed: true`, an `action`, and the updated source-neutral `TaskItem`.

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

For Todoist tasks, `task state todoist:<remote-id> done` closes the task and
`task state todoist:<remote-id> open` reopens it. Other Todoist states fail.
