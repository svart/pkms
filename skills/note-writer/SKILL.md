---
name: note-writer
description: Write or revise stylistically consistent org-mode notes in this org-roam PKMS. Use this skill when drafting new notes, improving existing notes, filling stubs, structuring note sets, or adding internal links. Coordinate with pkms-manager to inspect existing notes, resolve UUIDs, validate links, and avoid duplicate notes.
---

# Note-writer

Use this skill to write, revise, and connect org-mode notes in the local PKMS style. The goal is a readable note that fits the existing graph: clear definitions, justified links, valid org syntax, and no duplicate concept notes.

## Before Writing

Inspect the database before creating or editing notes:

```bash
pkms resolve --title "<term>"
pkms query "<term>"
pkms get <uuid> --no-content
```

Use `pkms-manager` for command details and JSON schemas. Read candidate notes before linking to them. Do not create a new note when an existing note already covers the concept under a title, alias, or close variant.

If the user gives source files or existing notes, inspect them first. If the task is only analysis, do not edit the notes database.

## Reference Map

Load only the references needed for the current task:

- `references/style.md`: prose style, openings, headings, bold text, quotes, code, and tables.
- `references/linking.md`: link justification, UUID resolution, bidirectional links, overlinking, and link candidate decisions.
- `references/workflows.md`: workflows for new notes, existing notes, stub repair, and validation.
- `references/subgraphs.md`: connecting isolated clusters through bridge notes and preserving graph shape.

For most note-writing tasks, read `references/style.md` and `references/linking.md`. For creation, stub repair, or validation-heavy work, also read `references/workflows.md`. Read `references/subgraphs.md` only when connecting disconnected clusters or designing bridge paths.

## Core Workflow

1. Identify the note type: new note, existing note revision, stub repair, link cleanup, or subgraph connection.
2. Resolve existing concepts with `pkms resolve` and `pkms query`; inspect likely matches with `pkms get`.
3. Write or revise the note using the local style rules.
4. Add only locally justified inline links using `[[id:<uuid>][description]]`.
5. Validate each changed note and fix broken links, duplicate links, missing titles, and obvious orphans.

## Note Design

Prefer small, focused notes. Split "why" and "how" when a concept has both theoretical and practical sides. Use separate distinction notes when the main value is a boundary between related concepts.

Core concept notes open with a concise definition and boundary statement. Practical notes describe concrete principles, usage, or failure modes. Hub notes are indexes; they may contain flat link lists, but ordinary notes should place links inline in prose.

## Link Discipline

A link belongs only when the surrounding sentence is genuinely about the linked concept. Prefer the most general matching note unless the sentence is explicitly about a narrower variant. Avoid links to broad hubs from child notes, duplicate links to the same target, and internal links that duplicate the value of an external reference.

Check salient graph-worthy terms as link candidates. Leave general words, passing mentions, and incidental platform examples unlinked unless they are central to the note.

## Completion Criteria

A changed note is ready when:

- Internal links are valid.
- The note has a clear title and useful body content.
- Inline links are justified by nearby prose.
- Obvious duplicate notes or duplicate links have been resolved.
- The note is connected to the graph unless it is intentionally standalone.

Use `pkms validate <uuid>` for changed notes. Use `pkms check` when broader graph integrity may be affected.
