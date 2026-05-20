# Workflows

Use this reference for note creation, note revision, stub repair, and validation.

## New Note Set

1. Fetch or inspect the source material.
2. Identify distinct notes: core concepts, practical how-to notes, why/rationale notes, distinction notes, and hubs.
3. Resolve existing titles and aliases with `pkms resolve` and `pkms query`.
4. Create only missing notes:

```bash
pkms new "prefix Title" --create
```

5. Resolve the created UUIDs:

```bash
pkms resolve --title "prefix" --output-format json
```

6. Write content using `references/style.md`.
7. Add links using `references/linking.md`.
8. Validate each changed note.

## Existing Note Revision

1. Read the current note and its immediate graph context.
2. Preserve useful existing links and IDs.
3. Fix structure, prose, headings, and broken links without broad unrelated rewrites.
4. Resolve new link candidates before inserting UUID links.
5. Validate the changed note.

Use focused edits unless the user asked for a full rewrite.

## Stub Repair

A stub is an obvious placeholder or dead-end note: only a title, a single resource list, boilerplate, or no useful body content.

Fill stubs immediately when they block the current task:

1. Read any external resources already linked from the stub.
2. Inspect nearby or similarly named notes.
3. Write a compact but useful body: definition, boundary, and relevant relationships.
4. Add at least one justified internal link when a real graph connection exists.
5. Validate the note.

Do not leave placeholder sections behind.

## Link Cleanup

Use this workflow when the task is about broken links, overlinks, duplicates, or graph hygiene:

1. Run `pkms validate <uuid>` for each affected note.
2. Inspect reported targets and nearby prose.
3. Remove low-value links before adding new ones.
4. Resolve replacement targets by title and query.
5. Re-run validation.

## Validation

For changed notes:

```bash
pkms validate <uuid>
```

For broader graph changes:

```bash
pkms check
```

If the change affects titles, aliases, IDs, or graph connectivity, also inspect relevant paths, backlinks, or search results with `pkms-manager` commands.

The final state should have valid internal links, useful prose, no obvious duplicates, and no accidental orphaning.
