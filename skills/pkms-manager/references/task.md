# pkms task

Task-first namespace for local PKMS tasks.

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda week
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task p5 show
pkms task p5 open
pkms task p5 state WAITING
pkms task p5 done
pkms task p5 done --dry-run
```

PKMS task IDs can be written as bare canonical IDs, `p<ID>`, or `pkms:<ID>`.
Provider-backed tasks use stable `<source>:<remote-id>` IDs, such as
`todoist:<remote-id>`. Unknown provider IDs fail unless that provider is
configured in the current build.

Explicit Todoist and mixed-source task views use the same table shape plus a
`Project` column:
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
pkms task agenda week type:SCHED project:Alpha
pkms task agenda source:all date:overdue tags:!waiting
pkms task agenda source:all date:today,overdue
pkms task agenda upcoming --days 14 source:all priority:B
pkms task list 'scope:Some Note Title' after:2026-05-01 before:"2026-05-25 18:00"
pkms task list source:todoist 'todoist.filter:today | overdue'
```

`task <ID> state` changes only the TODO keyword. Valid states come from
configured `open_todo_states` and `closed_todo_states`. State input is
case-insensitive, but the file is written with the canonical config spelling.

`task <ID> done` is shorthand for the first configured closed state, defaulting
to `DONE` if none is configured.

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
pkms task todoist:<remote-id> show
pkms task add "Capture local task"
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent task"
pkms task add source:todoist "Buy milk tomorrow"
pkms task add source:todoist title:"Call Alice" due:2026-05-24 tag:phone priority:B
pkms task add source:todoist title:"Call Alice" sch:tod tag:phone prio:B
pkms task todoist:<remote-id> done
pkms task p<canonical-id> postpone --to 2026-06-01
pkms task todoist:<remote-id> postpone --to tomorrow
pkms task p<canonical-id> mod sch:2026-05-24
pkms task todoist:<remote-id> mod sch:
pkms task p<canonical-id> mod dl:2026-05-30
pkms task p<canonical-id> mod dep:<parent-id>
pkms task todoist:<remote-id> mod dl:
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
`* Inbox` heading. New daily notes are created under `daily_notes_dir`, or
`new_notes_dir` when `daily_notes_dir` is unset.

Default `task add` appends a TODO heading to the configured PKMS inbox note.
Use positional text or add modifiers. `note:` is PKMS-only and chooses the note
to append into. `dep:`/`depend:` is PKMS-only and appends the new task as the
final child heading in the referenced PKMS task's subtree:

```bash
pkms task add "Capture local task"
pkms task add title:"Call Alice" due:2026-05-24 deadline:2026-05-30 tag:phone priority:B
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add title:"Waiting on Alice" status:WAITING
pkms task add note:"Project Alpha" title:"Follow up" schedule:tomorrow
pkms task add dep:2 title:"Follow up on parent task"
```

Use positional text with `task add source:todoist` for Todoist Quick Add
natural-language parsing. Use structured creation for deterministic assistant
tasks:

```bash
pkms task add source:todoist title:"Call Alice" due:2026-05-24 deadline:2026-05-30 project:inbox tag:phone priority:B desc:"Discuss migration plan"
pkms task add source:todoist title:"Call Alice" sch:tod dead:tom project:inbox tag:phone,migration prio:B desc:"Discuss migration plan"
```

Add modifiers: `source:`/`src:`, `title:`, PKMS-only `status:`,
`tag:`/`tags:`/`label:`/`labels:`, `schedule:`/`sch:`/`sched:`/`due:`,
`deadline:`/`dead:`/`dl:`, `project:`/`proj:`, `prio:`/`priority:`/`pri:`,
`desc:`/`description:`/`body:`, and PKMS-only `note:` and `dep:`/`depend:`.

Structured due and deadline values accept unambiguous case-insensitive prefixes
of `today`, `tomorrow`, or weekday names, plus `YYYY-MM-DD` or
`YYYY-MM-DD HH:MM`. Weekday names resolve to the next upcoming matching weekday.
Priority must be `A`, `B`, or `C`. Repeat or comma-separate tag modifiers for
multiple labels. `project:` accepts either a Todoist project id or an exact
project name. If a project name is duplicated case-insensitively, use the
project id. Todoist task creation rejects `status:`, `note:`, and
`dep:`/`depend:`.

Todoist descriptions containing `pkms:id:<uuid>` PKMS note markers are detected
by `task list source:todoist` and `task todoist:<remote-id> show`.

Use `task list projects` and `task list tags` when you need task metadata. With
`source:pkms`, projects come from note-level or heading-level `PROJECT`
properties, and tags combine note `#+filetags` with heading tags. With
`source:todoist`, projects and tags come from Todoist
metadata. `source:all` combines both sources. Todoist task output uses a
human-readable `project` when metadata is available and keeps the raw id in
`project_id`.

JSON output from `task add source:todoist` is a creation wrapper with
`created: true` and the created source-neutral `TaskItem` in `item`. Use
`item.display_id` for confirmation to the user and `item.source_id` for the raw
Todoist id.

Use `--dry-run` before completing Todoist tasks when operating on a real token:

```bash
pkms task todoist:<remote-id> done --dry-run
```

Use stable `todoist:<remote-id>` ids for Todoist mutations. `task <ID>
postpone` accepts `--to tomorrow` or `--to YYYY-MM-DD` and works only for
recurring PKMS or Todoist tasks; non-recurring tasks fail. `task <ID> mod`
accepts add-style modifiers such as `title:`, `tag:`, `project:`, `prio:`,
`desc:`, `sch:tomorrow`, `dl:YYYY-MM-DD`, or PKMS-only `status:WAITING` and
`dep:<task-id>`.
Empty values clear metadata: `tag:`, `sch:`, `dl:`, `prio:`, `project:`, and
`desc:`. For PKMS, `dep:` removes the current parent task dependency. Schedule
and deadline edits work for PKMS and Todoist tasks. For PKMS,
`status:` rewrites the org TODO keyword and `dep:`/`depend:` moves the whole
task subtree to the end of the referenced task's subtree when a task ID is
provided. If no properties change, `mod` prints `Nothing changed` and exits
nonzero. Todoist `task mod` rejects `status:`. Todoist `task <ID> state`
supports only `open` and `done`.
