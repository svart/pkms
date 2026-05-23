# pkms Commands Overview

Use `pkms <command> --help` for exact current flags. This page is a compact map
for choosing commands.

## Health

```bash
pkms check
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

## Lookup and Search

```bash
pkms resolve --title <term>
pkms resolve --uuid <uuid-fragment>
pkms resolve --tags "tag1,tag2"
pkms resolve --todos
pkms query "terms"
pkms query "terms" --title
pkms query "terms" --tags
pkms query "terms" --content
pkms query "terms" --todos
```

`--embed` for `query` and `suggest` exists only in builds made with the
`embed` feature.

## Inspect and Navigate

```bash
pkms get <target>
pkms get <target> --links
pkms get <target> --headings --no-content
pkms path <from> <to>
pkms context <target> --depth 2 --max-tokens 4000
```

## Create and Repair

```bash
pkms new "Title"
pkms new "Title" --create --tags "tag1,tag2"
pkms new "Existing Note" --create --heading "Heading"
pkms fix <broken-full-uuid> <replacement-full-uuid>
pkms fix <broken-full-uuid> <replacement-full-uuid> --apply
```

`fix` requires full UUIDs for both arguments.

## Tasks

```bash
pkms todo
pkms agenda
pkms show <canonical-id>
pkms open <canonical-id>
pkms task list
pkms task agenda
pkms task today
pkms task overdue
pkms task upcoming --days 7
pkms task show p<canonical-id>
pkms task open p<canonical-id>
pkms task state p<canonical-id> WAITING
pkms task done p<canonical-id>
pkms task list source:todoist
pkms task agenda --today source:todoist
pkms task agenda --week source:all
pkms task today source:all
pkms task report --today --source all --output-format json
pkms task plan --today --source all --output-format json
pkms task inbox
pkms task list projects source:todoist
pkms task list tags source:todoist
pkms task show todoist:<remote-id>
pkms task add --source todoist "Buy milk tomorrow"
pkms task add --source todoist --title "Call Alice" --due 2026-05-24 --priority B
pkms task add --source todoist --title "Call Alice" --note "Project Alpha"
pkms task done todoist:<remote-id>
pkms task postpone todoist:<remote-id> --to tomorrow
pkms task schedule todoist:<remote-id> --due none
```

## Statistics

```bash
pkms stats
pkms stats --hubs
pkms stats --tags
pkms stats --todos
pkms orphans
```
