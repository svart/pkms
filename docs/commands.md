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
pkms path <from> <to>
pkms context <target> --depth 2
pkms context <target> --max-tokens 2000
```

Targets may be UUIDs, note titles, or file paths unless a command says
otherwise.

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
pkms todo
pkms agenda
pkms show <id>
pkms open <id>
pkms task list
pkms task agenda
pkms task today
pkms task overdue
pkms task upcoming --days 7
pkms task show p<id>
pkms task open p<id>
pkms task state p<id> WAITING
pkms task done p<id>
pkms task list source:todoist
pkms task agenda --today source:todoist
pkms task agenda --week source:all
pkms task today source:all
pkms task report --today --source all --output-format json
pkms task plan --today --source all --output-format json
pkms task inbox
pkms task projects source:todoist
pkms task labels source:todoist
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule todoist:<remote-id> --due none
pkms task update todoist:<remote-id> --title "Call Alice" --project Work --priority A
pkms task delete todoist:<remote-id> --dry-run
pkms task reopen todoist:<remote-id>
```

See [TODO and Agenda](todo-agenda.md) for task IDs, filtering, table columns,
editor behavior, and task state changes.

Todoist task reads require a build with `--features todoist` and a token from
`TODOIST_API_TOKEN` or `[todoist].token` in config. Prefer the environment
variable unless the config file is private and not committed.

## Configuration

```bash
pkms info
pkms init-config
pkms init-config --db ~/Documents/org
```
