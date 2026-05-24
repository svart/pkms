# pkms task

Task-first namespace for local PKMS tasks.

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task show p5
pkms task open p5
pkms task state p5 WAITING
pkms task done p5
pkms task done p5 --dry-run
```

PKMS task IDs can be written as bare canonical IDs, `p<ID>`, or `pkms:<ID>`.
Top-level compatibility commands use bare numeric IDs.

By default, `task list` mirrors `todo` output and `task agenda` mirrors
`agenda` output for PKMS tasks. Explicit Todoist and mixed-source task views
use the same table shape plus a `Project` column:
`Id,Date,State,Type,Prio,Tags,Project,Note,Heading`. With only one source, the
`Id` column is the bare source id. With `source:all`, local PKMS ids use
`p<ID>` and Todoist ids use `t<remote-id>`. Source-neutral JSON keeps stable
`display_id` and `source_id` fields.

Item-producing task commands accept positional string filters after the
subcommand. Metadata commands, `task list projects` and `task list tags`, accept
only source filters:

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

`task state` changes only the TODO keyword. Valid states come from configured
`open_todo_states` and `closed_todo_states`. State input is case-insensitive,
but the file is written with the canonical config spelling.

`task done` is shorthand for the first configured closed state, defaulting to
`DONE` if none is configured.

Todoist read support is available only in builds made with `--features todoist`:

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
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --label phone --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task postpone p<canonical-id> --to 2026-06-01
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule p<canonical-id> --due 2026-05-24
pkms task schedule todoist:<remote-id> --due none
pkms task deadline p<canonical-id> --deadline 2026-05-30
pkms task deadline todoist:<remote-id> --deadline none
```

Use stable `todoist:<remote-id>` IDs for Todoist mutations.
Never print the Todoist token. It is read from `TODOIST_API_TOKEN` by default,
or from `[todoist].token` in config when the environment variable is unset.

For Todoist-backed agenda views, bare `task agenda source:todoist` uses the
Todoist `!no date` filter to show only scheduled tasks. `today` uses `today`,
`overdue` uses `overdue`, `week` uses `next 7 days`, and `upcoming --days N` uses
`due after: today & next N days`. `todoist.filter:<query>` overrides those generated agenda
filters when the assistant needs custom Todoist syntax.

Prefer stable shortcuts for common assistant requests: `task agenda today`,
`task agenda overdue`, `task agenda upcoming --days N`, and `task inbox`. The first three
accept `source:pkms`, `source:todoist`, or `source:all`; `task inbox` defaults
to the PKMS inbox note configured as `[tasks].inbox`. Use
`task inbox source:todoist` for Todoist's `#Inbox` filter. When `[tasks].inbox`
is `daily`, PKMS inbox tasks live in today's daily note under the top-level
`* Inbox` heading.

Default `task add` appends a TODO heading to the configured PKMS inbox note.
Use positional text or structured fields:

```bash
pkms task add "Capture local task"
pkms task add --title "Call Alice" --due 2026-05-24 --deadline 2026-05-30 --label phone --priority B
```

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

Use `task list projects` and `task list tags` when you need task metadata. With
`source:pkms`, projects come from note-level or heading-level `PROJECT`
properties, and tags combine note `#+filetags` with heading tags as in `todo`
and `agenda`. With `source:todoist`, projects and tags come from Todoist
metadata. `source:all` combines both sources. Todoist task output uses a
human-readable `project` when metadata is available and keeps the raw id in
`project_id`.

JSON output from `task add --source todoist` is a creation wrapper with
`created: true` and the created source-neutral `TaskItem` in `item`. Use
`item.display_id` for confirmation to the user and `item.source_id` for the raw
Todoist id.

Use `--dry-run` before completing Todoist tasks when operating on a real token:

```bash
pkms task done todoist:<remote-id> --dry-run
```

Use stable `todoist:<remote-id>` ids for Todoist mutations. `task postpone`
accepts `--to tomorrow` or `--to YYYY-MM-DD` and works only for recurring PKMS
or Todoist tasks; non-recurring tasks fail. `task schedule` accepts `--due
tomorrow`, `--due YYYY-MM-DD`, or `--due none`. `task deadline` accepts
`--deadline tomorrow`, `--deadline YYYY-MM-DD`, or `--deadline none`. Schedule
and deadline work for PKMS and Todoist tasks. Todoist `task state` supports only
`open` and `done`.
