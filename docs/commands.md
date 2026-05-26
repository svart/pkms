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
pkms check --attachment-links
pkms check --filetags
pkms check --agenda
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
pkms validate <target>
```

`check` scans the database and returns exit code 1 when issues are found.
`validate` checks one note or reads targets from NDJSON stdin with
`--from-stdin`.

## Lookup and Search

```bash
pkms resolve --uuid <uuid>
pkms resolve --title <title>
pkms resolve --tags "tag1,tag2"
pkms resolve --todos
pkms query "search terms"
pkms query "search terms" --title
pkms query "search terms" --tags
pkms query "search terms" --content
pkms query "search terms" --todos
```

When built with `--features embed`:

```bash
pkms query "search terms" --embed
```

## Navigation

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --headings --no-content
pkms serve <target>
pkms serve <target> --port 0
pkms path <from> <to>
pkms context <target> --depth 2
pkms context <target> --max-tokens 2000
```

Targets may be UUIDs, note titles, or file paths unless a command says
otherwise.

`serve` starts a foreground local HTTP server and renders the selected note as
HTML. Internal `id:` links navigate to `/?id=<uuid>`, so the browser address bar
tracks the rendered note. Local `file:` links and `attachment:` links are served
as assets when they resolve under the database root or the user's home
directory; image assets are embedded in the page. Org tables are rendered as
HTML tables, source blocks are syntax-highlighted for common languages, and TeX
formula text is shown as styled math without client-side JavaScript.

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
pkms fix <broken-uuid> <replacement-uuid>
pkms fix <broken-uuid> <replacement-uuid> --apply
```

`fix` is a dry run unless `--apply` is present.

## Suggestions

```bash
pkms suggest <uuid>
pkms suggest <uuid> --limit 5
pkms suggest <uuid> --exclude-orphans
```

When built with `--features embed`:

```bash
pkms suggest <uuid> --embed
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
pkms task list prio:A,B,C
pkms task list --columns +Project
pkms task agenda --columns -Project
pkms task agenda week type:SCHED project:Alpha
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
pkms task add title:"Call Alice" sch:tom dead:2026-05-30 tag:phone prio:B
pkms task add note:"Project Alpha" title:"Follow up"
pkms task add source:todoist "Buy milk tomorrow"
pkms task add source:todoist title:"Call Alice" due:2026-05-24 priority:B
pkms task add source:todoist title:"Call Alice" sch:tod tag:phone prio:B
pkms task todoist:<remote-id> done
pkms task p<id> postpone --to 2026-06-01
pkms task todoist:<remote-id> postpone --to tomorrow
pkms task p<id> schedule --due 2026-05-24
pkms task todoist:<remote-id> schedule --due none
pkms task p<id> deadline --deadline 2026-05-30
pkms task todoist:<remote-id> deadline --deadline none
```

See [TODO and Agenda](todo-agenda.md) for task IDs, filtering, table columns,
editor behavior, and task state changes.

`pkms task inbox` and default `pkms task add` use the PKMS inbox note configured
as `[tasks].inbox`. The value can be a note title, UUID, absolute path, or path
relative to `db_root`. Set it to `daily` to use today's daily note and place
new tasks under `* Inbox`. New daily notes are created under `daily_notes_dir`,
or `new_notes_dir` when `daily_notes_dir` is unset.

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
