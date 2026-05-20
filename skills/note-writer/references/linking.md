# Linking Reference

Use this reference when adding, revising, or validating internal links.

## Link Placement

Links belong inline in the prose, on the phrase that names the related concept:

```org
Unlike [[id:uuid][filters]], widgets preserve the full structure of the input.
```

Do not tack links onto the end of paragraphs. Avoid "See also" sections in ordinary notes. Hub notes are the exception because their purpose is to enumerate related notes.

## Link Justification

A link must be justified by the immediate sentence. The linked concept should be the direct object of what the sentence is about; the sentence should be weaker, incomplete, or less navigable without the link.

Useful rules:

- Prefer the most general matching note unless the sentence names a narrower variant.
- Check aliases and close variants before creating a note.
- Do not link child notes directly to broad hubs when a parent topic note is the better target.
- Do not enumerate platforms or ecosystems on general concept notes unless the note is specifically about that comparison.
- Do not create intermediate notes just to preserve a broken link. Remove the link if no real target exists.
- Prefer external URLs for reference and background context when an internal note adds no navigational value.
- Keep one internal link per sentence in normal prose.

Remove links when the target is the current note, the description is only a filename or incidental proper noun, or another nearby element already provides the same context.

## Candidate Resolution

For salient graph-worthy terms, resolve existing notes before creating links:

```bash
pkms resolve --title "<concept>"
pkms query "<concept>"
pkms get <uuid> --no-content
```

Inspect likely matches before linking. If several notes match, link to the one whose scope matches the sentence. If no match exists, create a note only when the concept is substantial enough to stand on its own.

Substantial concepts include distinct technologies, protocols, system calls, standards, design patterns, architectural principles, and recurring domain concepts.

Usually leave these unlinked:

- General terms already explained in the current note.
- Passing mentions with no further context.
- Platform names used only as examples.
- Concepts that would be better represented by an external URL.

## Bidirectional Links

A bidirectional link is justified only when both notes independently need each other for context. This usually means overlapping but non-nested counterparts, such as two platform-specific mechanisms that explain each other by contrast.

Most parent-child relationships need only one direction. Choose the direction that serves the reader's flow.

## Overlinking

After adding links, run:

```bash
pkms validate <uuid>
```

If a note links to the same target repeatedly, keep the link where the target is the direct object of the sentence. Remove duplicates from incidental mentions. Tables can legitimately repeat links when each row is an independent comparison item.

## Broken Links

When validation reports a broken internal link:

1. Resolve the intended concept by title and query.
2. If a real target exists, update the UUID.
3. If no real target exists and the concept does not deserve a new note, remove the link.
4. If the concept deserves a note, create it, write useful content, and link to it.
