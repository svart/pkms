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

`serve` exists only in builds made with the `web` feature:

```bash
pkms serve <target>
```

It starts a foreground local web viewer. Internal links navigate with
`/?id=<uuid>`; local file and attachment links are served as static assets when
they resolve to allowed local paths.

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
pkms task list
pkms task agenda
pkms task agenda today
pkms task agenda overdue
pkms task agenda upcoming --days 7
pkms task list state:TODO tags:work,!blocked
pkms task agenda week type:SCHED project:Alpha
pkms task p<canonical-id> show
pkms task p<canonical-id> open
pkms task p<canonical-id> state WAITING
pkms task p<canonical-id> done
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
pkms task p<canonical-id> postpone --to 2026-06-01
pkms task todoist:<remote-id> postpone --to tomorrow
pkms task p<canonical-id> schedule --due 2026-05-24
pkms task todoist:<remote-id> schedule --due none
pkms task p<canonical-id> deadline --deadline 2026-05-30
pkms task todoist:<remote-id> deadline --deadline none
```

## Statistics

```bash
pkms stats
pkms stats --hubs
pkms stats --tags
pkms stats --todos
pkms orphans
```
