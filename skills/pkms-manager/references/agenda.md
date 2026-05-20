# pkms agenda

List planned task headings with SCHEDULED or DEADLINE timestamps.

```bash
pkms agenda
pkms agenda --today
pkms agenda --week
pkms agenda --overdue
pkms agenda --upcoming
pkms agenda --date 2026-01-01
pkms agenda --columns Id,Date,State,Type,Prio,Tags,Note,Heading
pkms agenda --state "TODO"
pkms agenda --tags "!private,project"
pkms agenda --type "SCHED,DEADL"
pkms agenda --prio A
pkms agenda --sort "date,priority"
```

Use `show <Id>` to inspect a listed task and `open <Id>` to jump to it in an
editor.
