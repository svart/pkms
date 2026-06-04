# Maintenance Workflows

## Health Check After Edits

```bash
pkms validate <changed-note>
pkms check
```

Use `validate` for focused feedback on a changed note and `check` for whole
database health.

## Fix Broken ID Links

```bash
pkms check --id-links
pkms resolve --title "intended target"
pkms fix <broken-uuid> <replacement-uuid>
pkms fix <broken-uuid> <replacement-uuid> --apply
pkms check --self-links --id-links
```

Before applying `fix`, inspect the source link text. The same broken UUID can
appear in different contexts and may not always map to the same replacement.

## Check Link Quality

```bash
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
```

Use these checks when cleaning up dense notes or after broad link replacement.

## Integrate Orphans

```bash
pkms orphans
pkms get <orphan> --links
pkms query <related_term> --limit 20
pkms suggest <orphan> --limit 20
pkms validate <orphan>
pkms check
```

Add links only where the surrounding sentence justifies the relationship. Do
not mechanically add backlinks.

## Scope Work with Pipelines

```bash
pkms resolve --tags "project" --output-format ndjson | pkms task list --from-stdin
pkms query "topic" --output-format ndjson | pkms validate
```

Use NDJSON pipelines for batch inspection and validation.

## Terminal Task Workflow

Use the local `task` namespace when you want task-first commands and explicit
state changes:

```bash
pkms task list
pkms task agenda today
pkms task p5 show
pkms task p5 state WAITING
pkms task p5 done --dry-run
pkms task p5 done
```

Use positional filters such as `scope:`, `state:`, `tags:`, and `type:` for
scoped task views. Use `task list --group` for grouped TODO output.

When built with Todoist support, phone-captured Todoist tasks can be included in
terminal task review:

```bash
pkms task list source:todoist 'todoist.filter:today | overdue'
pkms task list source:all
```

Use stable remote IDs for Todoist task inspection:

```bash
pkms task todoist:<remote-id> show
```

Capture to Todoist from the terminal when phone capture is the intended
cross-device workflow:

```bash
pkms task add source:todoist "Buy milk tomorrow"
pkms task todoist:<remote-id> done --dry-run
pkms task todoist:<remote-id> done
```
