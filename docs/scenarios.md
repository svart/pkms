# Scenario Guide

This page routes common user and agent workflows to the right command family
and detailed documentation.

## Note Database Commands

Use note database commands when the task is about the org-roam graph itself:
health, links, lookup, search, note inspection, graph navigation, note
creation, heading extraction, or repair.

Common workflows:

```bash
pkms check
pkms validate <changed-note>
pkms resolve --title "project alpha"
pkms query "distributed systems" --limit 10
pkms get <uuid-or-title> --links
pkms path "Source Note" "Target Note"
pkms stats --hubs 20
pkms orphans --limit 20
pkms new "New Note" --create --tags "project,idea"
pkms extract <heading-uuid> "New Note Title" --apply
pkms fix uuid <broken-uuid> <replacement-uuid> --apply
pkms fix attach --apply
```

Start with [Note Database Commands](note-database-commands.md). Use
[Command Reference](commands.md) for flags, [Notes Database Format](database-format.md)
for source file expectations, and [Maintenance Workflows](workflows.md) for
repeatable cleanup recipes.

## Task Commands

Use `pkms task` when the work is task-first: agenda views, inbox capture,
task inspection and local org task mutations.

Common workflows:

```bash
pkms task list
pkms task agenda today
pkms task agenda overdue
pkms task list state:TODO tags:work,!blocked prio:A
pkms task 5 show
pkms task 5 done --dry-run
pkms task 5 done
pkms task add title:"Call Alice" sch:tom tag:phone prio:B
pkms task add dep:5 title:"Follow up"
```

Start with [TODO and Agenda](todo-agenda.md). Use
[Task System Design](task-system.md) before changing task IDs, filters,
providers, source semantics, or mutations.

## Web Viewer

Use `pkms serve` when a human needs to read a note and navigate linked notes in
a browser while keeping the source database local.

```bash
cargo run --features web -- --db ~/Documents/org serve "Project Alpha"
pkms serve <uuid-or-title> --port 0
```

Start with [Web Viewer](web.md). Implementation details live in the
[pkms-web crate docs](crates/pkms-web.md).

## RAG Retrieval

Use `pkms rag` in builds made with `--features rag` when the workflow needs
local retrieval over note chunks, source citations, search output for another
tool, or a local retrieval HTTP API/UI.

```bash
pkms rag index
pkms rag status
pkms rag search "agenda inspect tasks" --limit 5
pkms rag retrieve "agenda inspect tasks" --limit 5 --mode hybrid
pkms tags
pkms tags suggest 11111111-1111-4111-8111-111111111111
pkms tags suggest 12 --apply
pkms rag serve --host 127.0.0.1 --port 7337
```

Start with [RAG Retrieval](rag.md). Implementation details live in the
[pkms-rag crate docs](crates/pkms-rag.md).

## Structured Output and Pipelines

Use JSON when another process needs a full response object. Use NDJSON when
streaming records between commands:

```bash
pkms query "rust" --output-format ndjson | pkms get --links --from-stdin
pkms resolve --tags "project" --output-format ndjson | pkms task list --from-stdin
```

See [Pipelining](pipelining.md) and [JSON and NDJSON Output](json-output.md).
