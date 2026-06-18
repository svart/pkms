# Command Reference

Run `pkms <command> --help` for the authoritative flag list. This page groups
the main commands and common options.

## Global Flags

| Flag | Description |
|------|-------------|
| `--db PATH` | Path to org-roam database root. Overrides config. |
| `--output-format FMT` | `text`, `json`, or `ndjson`. |

## Health

```bash
pkms check
pkms check --stats
pkms check --id-links
pkms check --file-links
pkms check --remote-file-links
pkms check --attachment-links
pkms check --filetags
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
pkms validate <target>
```

`check` scans the database and returns exit code 1 when issues are found.
`validate` checks one note or reads targets from NDJSON stdin with
`--from-stdin`.

`check --remote-file-links` is explicit network access for SSH `file:` links
and requires a binary built with `--features ssh`. It implies file-link output
but does not imply attachment checks. Supported targets use TRAMP-style SSH
syntax with absolute remote paths:

```org
[[file:/ssh:host:/absolute/path]]
[[file:/ssh:user@host:/absolute/path]]
[[file:/ssh:user@host#222:/absolute/path::needle]]
```

Without `--remote-file-links`, SSH `file:` targets are skipped instead of being
treated as local absolute paths. `validate` is local-only and also skips SSH
file targets.

SSH checks use strict `known_hosts` verification and passwordless public-key
authentication only. A configured private key is tried first, then the SSH
agent when enabled. Password and keyboard-interactive prompts are not used.
Missing remote files are reported in `broken_file_links`; auth, host-key,
timeout, unsupported syntax, and other SSH/SFTP failures are reported in
`file_link_errors`.

## Lookup and Search

```bash
pkms resolve --uuid <uuid>
pkms resolve --title <title>
pkms resolve --tags "tag1,tag2"
pkms resolve --title <title> --todos
pkms query "search terms"
pkms query "search terms" --title
pkms query "search terms" --tags
pkms query "search terms" --content
pkms query "search terms" --todos
```

## Navigation

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --heading "Section title"
pkms get <target> --headings --no-content
pkms path <from> <to>
```

Targets may be UUIDs, note titles, or file paths unless a command says
otherwise.

When built with `--features web`:

```bash
pkms serve <target>
pkms serve <target> --port 0
```

`serve` starts a foreground local HTTP server and renders the selected note as
HTML. Internal `id:` links navigate to `/?id=<uuid>`, so the browser address bar
tracks the rendered note. Local `file:` links and `attachment:` links are served
as assets when they resolve under the database root or the user's home
directory; image assets are embedded in the page. Free-standing `http://` and
`https://` URLs in rendered text are clickable after org links are resolved. Org
tables are rendered as HTML tables, note and heading tags are shown as compact
chips, heading `SCHEDULED` and `DEADLINE` timestamps are shown as planning
badges, source blocks are highlighted server-side with Syntect, and TeX formula
text is rendered to static KaTeX HTML without client-side JavaScript.
The note header includes an "Open in Emacs" button that opens the rendered note
file through the same `emacsclient -n` editor path used by task opening.
The page also includes collapsible floating panels: note contents on the left
from the heading hierarchy, and backlinks on the top right from notes that link
to the current note. On wide viewports both panels open by default; on
constrained viewports they start collapsed. Opening a panel keeps it floating in
the nearest top corner and may cover the note text instead of changing the main
note column width. Hovering over an internal note link opens a scrollable note
preview after a short delay; links inside the preview remain clickable, and
clicking back in the original note closes the preview.

## Statistics and Discovery

```bash
pkms stats
pkms stats --days 30
pkms stats --hubs
pkms stats --hubs 20
pkms stats --tags
pkms stats --todos
pkms orphans
pkms orphans --with-dailies
pkms orphans --limit 20
```

## Creation and Repair

```bash
pkms new "My Note"
pkms new "My Note" --create
pkms new "My Note" --create --tags "tag1,tag2"
pkms new "My Note" --create --aliases "Alias1,Alias2"
pkms new "Existing Note" --create --heading "Heading"
pkms extract <heading-uuid>
pkms extract <heading-uuid> "New Note Title" --apply
pkms fix <broken-uuid> <replacement-uuid>
pkms fix <broken-uuid> <replacement-uuid> --apply
```

`extract` is a dry run unless `--apply` is present. It accepts a heading-level
UUID, creates a new note whose primary `:ID:` is that heading UUID, and replaces
the old subtree with an `id:` link heading. The optional new title changes only
the new note `#+title`; the replacement link label uses the original heading
title.

`fix` is a dry run unless `--apply` is present.

## Suggestions

```bash
pkms suggest <uuid>
pkms suggest <uuid> --limit 5
pkms suggest <uuid> --exclude-orphans
```

## Tasks

```bash
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task list --group state
pkms task list --from-stdin
pkms task list state:TODO tags:work,!blocked
pkms task list state:opened,!waiting
pkms task list prio:A,B,C
pkms task list date:fri
pkms task list after:tom before:"2026-06-19 12:00"
pkms task list --columns +Project
pkms task agenda --columns -Project
pkms task agenda week type:SCHED project:Alpha
pkms task agenda date:tom
pkms task p<id> show
pkms task p<id> open
pkms task p<id> state WAITING
pkms task p<id> done
pkms task list source:todoist
pkms task agenda today source:todoist
pkms task agenda week source:all
pkms task agenda today source:all
pkms task inbox
pkms task inbox source:todoist
pkms task list projects source:todoist
pkms task list tags source:todoist
pkms task todoist:<remote-id> show
pkms task add "Capture local task"
pkms task add title:"Call Alice" sch:mon dead:to tag:phone prio:B
pkms task add title:"Waiting on Alice" state:WAITING
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add dep:2 title:"Follow up on parent task"
pkms task add source:todoist "Buy milk tomorrow"
pkms task add source:todoist title:"Call Alice" due:2026-05-24 priority:B
pkms task add source:todoist title:"Call Alice" sch:tod tag:phone prio:B
pkms task todoist:<remote-id> done
pkms task p<id> postpone --to 2026-06-01
pkms task todoist:<remote-id> postpone --to tomorrow
pkms task p<id> mod sch:2026-05-24
pkms task p<id> mod state:WAITING
pkms task todoist:<remote-id> mod sch:
pkms task p<id> mod dl:2026-05-30
pkms task p<id> mod dep:<parent-id>
pkms task todoist:<remote-id> mod dl:
```

On ANSI-capable terminals, task text output renders inline `=code=`,
`~orange code~`, and mentions such as `@alice`. Piped text output keeps the
stored strings unchanged unless ANSI output is forced. JSON/NDJSON output is
unchanged.

See [TODO and Agenda](todo-agenda.md) for task IDs, filtering, table columns,
editor behavior, task state changes, and show output parent/child chains.

`pkms task inbox` and default `pkms task add` use the PKMS inbox note configured
as `[tasks].inbox`. The value can be a note title, UUID, absolute path, or path
relative to `db_root`. Set it to `daily` to use today's daily note and place
new tasks under `* Inbox`. New daily notes are created under `daily_notes_dir`,
or `new_notes_dir` when `daily_notes_dir` is unset.
Use `dep:<task-id>` or `depend:<task-id>` to add a PKMS task as the final child
heading of an existing PKMS task subtree.

Task schedule and deadline dates accept unambiguous prefixes of `today`,
`tomorrow`, or weekday names; weekdays resolve to the next upcoming matching
weekday.

Todoist task reads require a build with `--features todoist` and a token from
`TODOIST_API_TOKEN` or `[todoist].token` in config. Prefer the environment
variable unless the config file is private and not committed. HTTPS uses the
platform certificate verifier, so system trust-store corporate proxy roots are
honored.

## Configuration

```bash
pkms info
pkms init-config
pkms init-config --db ~/Documents/org
```
