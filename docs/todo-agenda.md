# TODO and Agenda

This page is the user-facing guide for task commands. For implementation
guardrails and source-model decisions, see
[Task System Design](task-system.md).

`task list`, `task agenda`, and task ID actions share one canonical task ID
space. The ID shown by `task list` is the same ID used by `task agenda`,
`task p<ID> show`, and `task p<ID> open`.

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

Provider-backed tasks use stable `<source>:<remote-id>` IDs, such as
`todoist:<remote-id>`. Unknown provider IDs parse as task IDs but fail at
execution unless that provider is configured in the current build.

## Task List

`task list` lists TODO headings.

```bash
pkms task list
pkms task list --columns Id,Date,State,Type,Prio,Tags,Project,Note,Heading
pkms task list --group state
pkms task list --sort file
pkms task list --from-stdin
pkms task list --line-sep
```

Use positional task filters for state, tags, type, priority, date range, and
scope:

```bash
pkms task list state:TODO tags:work,!blocked prio:A
pkms task list 'scope:Some Note Title' after:2026-05-01 before:"2026-05-25 18:00"
```

`task list` shows headings in configured open TODO states unless filters select
other states.

## Agenda

`task agenda` lists planned task headings with `SCHEDULED` or `DEADLINE`
timestamps.

```bash
pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task agenda date:upcoming
pkms task agenda date:2026-01-01
pkms task agenda date:today,overdue
pkms task agenda --columns Id,Date,State,Type,Prio,Tags,Project,Note,Heading
pkms task agenda state:TODO
pkms task agenda tags:!device,agenda
pkms task agenda type:SCHED,DEADL
pkms task agenda prio:A
pkms task agenda --sort "date,priority"
```

Use `date:upcoming` for all future, non-overdue planned tasks. Use
`task agenda upcoming --days N` for a bounded upcoming window.

## Filters

Comma-separated filters use AND logic. Prefix a value with `!` for negation.

```bash
pkms task list state:TODO
pkms task list state:!DONE,!CANCELLED
pkms task agenda tags:agenda,project
pkms task agenda tags:!private
pkms task agenda type:SCHED,DEADL
```

## Columns

Available table columns:

```text
Id,Date,State,Type,Prio,Tags,Project,Note,Heading
```

`--columns` can replace the active set or adjust it. For example,
`--columns Id,Heading` shows exactly those columns, `--columns +Project` adds
`Project` to the configured/default set, and `--columns -Project` removes it.
Unknown columns, duplicate columns, mixed replacement/adjustment syntax, adding
an already enabled column, and removing a disabled column are reported as
errors.

Use `--line-sep` for row separator lines in text output.

## Show and Open

Inspect a task by canonical ID:

```bash
pkms task p5 show
pkms task pkms:5 show
```

Open a task at its source heading:

```bash
pkms task p5 open
```

By default, `task p<ID> open` runs `emacsclient -n`. Override it with
`--editor` or open a specific line with `--line`.

```bash
pkms task p5 open --editor "emacsclient -n"
pkms task p5 open --line 42
```

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

PKMS, Todoist, and mixed-source task views use the standard task table columns:

```text
Id,Date,State,Type,Prio,Tags,Project,Note,Heading
```

Text task views render IDs for the selected source set. In source-neutral views
with only one source selected, `Id` is the bare source id. With `source:all`,
PKMS ids are prefixed with `p` and Todoist ids are prefixed with `t`.

Source-neutral JSON and NDJSON include fields such as `source`, `source_id`,
`display_id`, `status`, `state`, `note_title`, `note_uuid`, `path`, and
`line_number`. PKMS task items also include metadata such as
`has_agenda_tag`, `is_daily_file`, `daily_file_date`, and `heading_level`.

Item-producing task commands accept positional string filters after the
subcommand. These filters are separate from normal command flags such as
`--limit`, `--sort`, and `--columns`. Metadata commands, `task list projects`
and `task list tags`, accept only source filters.

| Filter | Syntax | Notes |
|--------|--------|-------|
| Source | `source:pkms`, `source:todoist`, `source:all` | Select local PKMS tasks, Todoist tasks, or both. Defaults to `source:pkms`. |
| Source alias | `src:pkms`, `src:todoist`, `src:all` | Short form of `source:`. |
| Todoist raw filter | `todoist.filter:<query>` | Uses Todoist's server-side filter endpoint. Requires `source:todoist` or `source:all`. |
| State | `state:TODO`, `state:opened`, `state:closed` | Matches a TODO state case-insensitively. `opened` and `closed` expand to configured open and closed task states. |
| State exclusion | `state:!DONE`, `state:!closed` | Excludes matching states. Comma-separated state filters use AND logic. |
| Tags | `tags:tag1,tag2` | Matches combined note filetags and heading tags for PKMS, and labels for Todoist. Tags are exact. |
| Tag alias | `tag:tag1,tag2` | Short form of `tags:`. |
| Tag exclusion | `tags:!tag1,tag2` | Excludes `tag1` and requires `tag2`. Comma-separated tag filters use AND logic. |
| Type | `type:SCHED`, `type:DEADL` | Matches scheduled or deadline tasks. `kind:` is an alias. |
| Type exclusion | `type:!SCHED` | Excludes tasks with a scheduled timestamp. |
| Priority | `prio:A`, `priority:A`, `prio:A,B,C` | Matches one or more priorities; matching is case-insensitive. |
| No priority | `prio:none`, `priority:none` | Matches tasks without a priority. |
| Exact agenda date | `date:YYYY-MM-DD` | Matches scheduled or deadline dates on that day. |
| Today | `date:today` | Matches scheduled or deadline dates today. |
| Week | `date:week` | Matches scheduled or deadline dates through the next 7 days. |
| Overdue | `date:overdue`, `overdue` | Matches overdue tasks. |
| Upcoming | `date:upcoming`, `upcoming` | Matches non-overdue tasks after today. |
| Multiple dates | `date:today,overdue,YYYY-MM-DD` | Matches any listed date filter. |
| After | `after:YYYY-MM-DD`, `after:YYYY-MM-DD HH:MM` | Matches tasks on or after the date/time. |
| Before | `before:YYYY-MM-DD`, `before:YYYY-MM-DD HH:MM` | Matches tasks on or before the date/time. |
| Scope | `scope:<target>` | Restricts PKMS tasks to a note title, UUID, or path. |
| Project | `project:<name-or-id>` | Matches PKMS `PROJECT` properties and Todoist project names or ids. |
| Project exclusion | `project:!<name-or-id>` | Excludes matching projects. |

Examples:

```bash
pkms task list state:TODO tags:work,!blocked prio:A
pkms task list state:opened,!waiting
pkms task agenda week type:SCHED project:Alpha
pkms task agenda source:all date:overdue tags:!waiting
pkms task agenda source:all date:today,overdue
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
pkms task todoist:<remote-id> show
pkms task add "Capture local task"
pkms task add title:"Call Alice" due:2026-05-24 deadline:2026-05-30 tag:phone priority:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent task"
pkms task add source:todoist "Buy milk tomorrow"
pkms task add source:todoist project:Inbox "Buy milk tomorrow"
pkms task add source:todoist title:"Call Alice" due:2026-05-24 tag:phone priority:B
pkms task add source:todoist title:"Call Alice" sch:tod tag:phone prio:B
pkms task todoist:<remote-id> done
pkms task todoist:<remote-id> done --dry-run
pkms task p<id> postpone --to 2026-06-01
pkms task todoist:<remote-id> postpone --to tomorrow
pkms task p<id> mod sch:2026-05-24
pkms task p<id> mod state:WAITING
pkms task todoist:<remote-id> mod sch:2026-05-24
pkms task todoist:<remote-id> mod sch:
pkms task p<id> mod dl:2026-05-30
pkms task p<id> mod dep:<parent-id>
pkms task todoist:<remote-id> mod dl:
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
Text output uses the same default agenda sections as PKMS tasks: overdue, today,
and upcoming.
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
the `* Inbox` heading, creating the heading when needed. New daily notes are
created under `daily_notes_dir`, or `new_notes_dir` when `daily_notes_dir` is
unset. Without an inbox configuration, PKMS inbox commands fail. `task inbox
source:todoist` continues to use Todoist's `#Inbox` filter.

PKMS task creation appends a TODO heading to that inbox note by default. It
accepts positional text or add modifiers. `note:` is PKMS-only and chooses the
note to append into. `dep:`/`depend:` is PKMS-only and adds the new task as a
child heading at the end of the referenced task's subtree:

```bash
pkms task add "Capture local task"
pkms task add title:"Call Alice" due:2026-05-24 deadline:2026-05-30 tag:phone priority:B
pkms task add title:"Call Alice" sch:tod dead:tom tag:phone prio:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add note:"Project Alpha" title:"Follow up" schedule:tomorrow
pkms task add dep:2 title:"Follow up on parent task"
```

Todoist task creation supports two modes. Positional text uses Todoist Quick Add
and lets Todoist parse natural language, labels, priorities, and projects:

```bash
pkms task add source:todoist "Buy milk tomorrow #Inbox @errand p1"
```

Structured creation uses Todoist API fields and is the safer mode for assistant
workflows:

```bash
pkms task add source:todoist title:"Call Alice" due:2026-05-24 deadline:2026-05-30 project:inbox tag:phone,migration priority:B desc:"Discuss migration plan"
pkms task add source:todoist title:"Call Alice" sch:tod dead:tom project:inbox tag:phone,migration prio:B desc:"Discuss migration plan"
```

Add modifiers mirror task filters where practical:

| Modifier | Aliases | Meaning |
|----------|---------|---------|
| `source:<pkms-or-todoist>` | `src:` | Select task source. |
| `title:<text>` | | Structured task title. Non-modifier words are task text. |
| `state:<state>` | | PKMS only; set the org TODO keyword from configured agenda states. |
| `tag:<label>` | `tags:`, `label:`, `labels:` | Add labels/tags. Values can be comma-separated. |
| `schedule:<date>` | `sch:`, `sched:`, `due:` | Set scheduled/due date. |
| `deadline:<date>` | `dead:`, `dl:` | Set deadline date. |
| `project:<name-or-id>` | `proj:` | Set project for Todoist tasks. |
| `prio:<A-B-C>` | `priority:`, `pri:` | Set source-neutral priority. |
| `desc:<text>` | `description:`, `body:` | Set description/body text. |
| `note:<target>` | | PKMS only; append to this note instead of the configured inbox. |
| `dep:<task-id>` | `depend:` | PKMS only; append as a child of the referenced PKMS task. |

`schedule:`/`deadline:` values accept unambiguous case-insensitive prefixes of
`today`, `tomorrow`, or weekday names, plus `YYYY-MM-DD` or
`YYYY-MM-DD HH:MM`. Weekday names resolve to the next upcoming matching weekday,
so `sch:fri` on a Friday means next Friday. Ambiguous prefixes fail with an
error. Modifier keys also accept unambiguous prefixes, so `proj:` and `pro:`
resolve to `project:` while `pr:` fails as ambiguous. Priorities use the
source-neutral `A`, `B`, or `C` convention.
Label modifiers can be repeated or comma-separated. `project:` accepts either a
Todoist project id or an exact project name. If a name matches multiple projects
case-insensitively, `pkms` fails before creating the task and asks for the
project id. Todoist task creation rejects `note:`.
Todoist task creation also rejects `state:` and `dep:`/`depend:`.

Listing or showing Todoist tasks detects `pkms:id:<uuid>` PKMS note markers in
Todoist descriptions and populates `note_uuid` and `note_title` when the note
exists locally.

Use `task list projects` and `task list tags` to inspect task metadata. With
`source:pkms`, projects come from note-level or heading-level `PROJECT`
properties, and tags combine note `#+filetags` with heading tags. With
`source:todoist`, projects and tags come from Todoist
metadata. `source:all` combines both sources. Todoist task output sets `project`
to the display name when metadata is available and keeps the raw id in
`project_id`.

`task add --output-format json` returns a wrapper with
`created: true` and the created source-neutral `TaskItem` under `item`, including
stable `display_id`, `source_id`, title, description, priority, dates, labels,
project, `project_id`, and URL when Todoist provides them. Text output stays concise but
includes the stable id and key planning fields.

ID-first `task <ID> mod` changes add-style task properties for PKMS and Todoist
tasks. For PKMS, `state:` updates the org TODO keyword and `sch:` and `dl:`
edit the heading planning line. For Todoist, `sch:` and `dl:` update `due_date`
and `deadline_date`. Empty values clear metadata: `tag:`, `sch:`, `dl:`,
`prio:`, `project:`, and `desc:`. For PKMS, `dep:<task-id>`/`depend:<task-id>`
moves the task's whole subtree, including body text and child headings, to the
end of the referenced task's subtree; `dep:` removes the current parent task
dependency. `task mod` is strict: task titles change only through `title:<text>`,
and unrecognized modifiers fail instead of becoming title text. Todoist
`task mod` rejects `state:` and `dep:`/`depend:`.
When changes are made, text output prints one diff line per changed property,
such as `Scheduled: Today (2026-06-02) -> Scheduled: Tomorrow (2026-06-03)`.
If no properties change, it prints `Nothing changed` and exits
nonzero. ID-first `task <ID> postpone` works only for recurring tasks. For PKMS,
the task must have a recurring `SCHEDULED` or `DEADLINE` timestamp and the
repeater/warning syntax is preserved. For Todoist, the Todoist due date must be
recurring. Non-recurring tasks fail instead of being silently rescheduled.
Changed-task JSON output returns `changed: true`, `changes`, and the updated
source-neutral `TaskItem`.

Set the token in the environment when possible. Environment variables take
precedence over config values and avoid storing the secret in a dotfile:

```bash
export TODOIST_API_TOKEN=...
```

Todoist HTTPS requests use the platform certificate verifier, so corporate proxy
root certificates installed in the system trust store are honored without
disabling certificate validation.

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

Todoist writes are explicit. `task add source:todoist` uses Todoist Quick Add
semantics, including natural language dates, labels, priorities, and project
syntax. `project:NAME` appends `#NAME` to the Quick Add text.

`task todoist:<remote-id> done` calls Todoist's close endpoint. Use `--dry-run`
to print the planned completion without sending a Todoist request.

## State Changes

`task <ID> state` changes only the TODO keyword on the target heading. Valid
states come from `open_todo_states` and `closed_todo_states` in config.

```bash
pkms task p5 state WAITING
pkms task pkms:5 state done
pkms task p5 done
pkms task p5 done --dry-run
```

State input is case-insensitive. The file is written with the canonical spelling
from config, so `done`, `Done`, and `DONE` all write `DONE` when `DONE` is the
configured state.

`task <ID> done` is shorthand for setting the first configured closed state. If
no closed state is configured, it defaults to `DONE`.

For Todoist tasks, `task todoist:<remote-id> state done` closes the task and
`task todoist:<remote-id> state open` reopens it. Other Todoist states fail.
