# Extract Heading to Note Plan

## Goal

Add a command that extracts a heading subtree into a new org-roam note while
preserving the extracted heading UUID as the new note UUID.

## Final Behavior Checklist

- [x] Add a command with this shape:
  `pkms extract <heading-uuid> [new-name] --apply`
- [x] Keep the command dry-run by default.
- [x] Require `--apply` before writing the source file or creating the new note.
- [x] Treat `<heading-uuid>` as a heading-level UUID only.
- [x] Reject note-level UUIDs with a clear error.
- [x] Reject unknown UUIDs with a clear error.
- [x] Reject ambiguous or duplicate UUID state with a clear error.
- [x] Locate the target heading through parsed heading metadata.
- [x] Extract the full subtree rooted at the target heading.
- [x] End the extracted subtree before the next heading with the same or lower
  level.
- [x] Create the new note under `ResolvedConfig::resolve_new_notes_dir()`.
- [x] Use the existing timestamp plus slug filename style from `pkms new`.
- [x] Use the extracted heading UUID as the new note primary `:ID:`.
- [x] If `new-name` is provided, use it only for the new note `#+title`.
- [x] If `new-name` is omitted, use the original extracted heading title for
  the new note `#+title`.
- [x] Keep the copied root heading text unchanged in the new note.
- [x] Remove only the copied root heading's own `:ID:` property.
- [x] Keep all child and subheading `:ID:` properties unchanged.
- [x] Keep non-ID properties on the copied root heading unchanged.
- [x] Remove an empty copied root heading properties drawer if removing `:ID:`
  leaves no properties.
- [x] Preserve the extracted subtree body, planning lines, TODO state, priority,
  tags, links, drawers, blocks, and child headings.
- [x] Replace the original source subtree with one heading at the original
  level.
- [x] Render the replacement heading as an ID link to the newly extracted note.
- [x] Preserve the original TODO state, priority, and tags around the
  replacement link.
- [x] Use the original heading title as the replacement link label, never
  `new-name`.

## Replacement Examples

- [x] Preserve TODO state and tags:

  ```org
  ** TODO Original Heading :tag:
  ```

  becomes:

  ```org
  ** TODO [[id:11111111-1111-4111-8111-111111111111][Original Heading]] :tag:
  ```

- [x] Preserve priority:

  ```org
  *** TODO [#A] Original Heading :tag:
  ```

  becomes:

  ```org
  *** TODO [#A] [[id:11111111-1111-4111-8111-111111111111][Original Heading]] :tag:
  ```

- [x] Promote only the extracted root heading UUID:

  ```org
  :PROPERTIES:
  :ID:       11111111-1111-4111-8111-111111111111
  :END:
  #+title: Better Note Title

  ** TODO Original Heading :tag:
  Body
  *** Child With Own ID
  :PROPERTIES:
  :ID:       22222222-2222-4222-8222-222222222222
  :END:
  More
  ```

## Implementation Checklist

- [x] Add `Extract(ExtractArgs)` to `src/cli.rs`.
- [x] Define `ExtractArgs` with `heading_uuid`, optional `new_name`, and
  `--apply`.
- [x] Add command dispatch for `extract` in `src/runner.rs`.
- [x] Add `pub mod extract;` in `src/commands/mod.rs`.
- [x] Add `src/commands/extract.rs`.
- [x] Add typed `ExtractOptions`.
- [x] Add typed `ExtractOutput` with `uuid`, `title`, `source_path`,
  `new_path`, `replacement`, `created`, and `applied`.
- [x] Expose or move note filename helpers currently used by `pkms new`.
- [x] Reuse the same filename collision behavior as `pkms new`.
- [x] Add a default-feature graph helper that maps heading UUID to source note
  UUID and line number.
- [x] Use the existing graph/parser data to locate the source path and heading.
- [x] Read the source file with `org_edit::read_lines`.
- [x] Compute subtree start from the target heading line.
- [x] Compute subtree end from parsed heading levels.
- [x] Build new note content with a top-level properties drawer, `#+title`, and
  the copied subtree.
- [x] Remove the root heading `:ID:` from only the copied subtree.
- [x] Leave child heading ID properties untouched.
- [x] Render the replacement heading from parsed heading components rather than
  string splicing.
- [x] In dry-run mode, compute output without writing files.
- [x] In apply mode, create the new note with exclusive create semantics.
- [x] In apply mode, replace the source subtree and write the source file.
- [x] Prefer creating the new note before rewriting the source file.
- [x] If source rewrite fails after new note creation, report both paths clearly.
- [x] Print human-readable text output for default format.
- [x] Print stable JSON output for `--output-format json`.
- [x] Do not emit NDJSON for this command unless a later requirement adds
  stream behavior.

## Test Checklist

- [x] Add integration test for dry-run with no file writes.
- [x] Add integration test for `--apply` creating the new note.
- [x] Add integration test for source subtree replacement.
- [x] Add integration test for default note title from heading title.
- [x] Add integration test proving `new-name` changes only `#+title`.
- [x] Add integration test proving the replacement link label uses the original
  heading title.
- [x] Add integration test for TODO state, priority, and tags in the replacement
  heading.
- [x] Add integration test proving the extracted root heading UUID is promoted
  to note-level `:ID:`.
- [x] Add integration test proving the copied root heading no longer has that
  `:ID:`.
- [x] Add integration test proving child heading IDs remain unchanged.
- [x] Add integration test for root heading properties drawer cleanup.
- [x] Add integration test for rejecting a note-level UUID.
- [x] Add integration test for rejecting an unknown UUID.
- [x] Add integration test for JSON output.
- [x] Add focused unit tests for subtree bound calculation.
- [x] Add focused unit tests for root heading ID removal.
- [x] Add focused unit tests for replacement heading rendering.

## Verification Checklist

- [x] Run focused extract tests:
  `cargo test --test integration extract`
- [x] Run focused unit tests for edited modules.
- [x] Run the fast pre-commit gate:
  `cargo fmt --check`
- [x] Run the fast pre-commit gate:
  `cargo clippy -- -D warnings`
- [x] Run the fast pre-commit gate:
  `cargo test`
- [x] Run the fast pre-commit gate:
  `cargo build`
