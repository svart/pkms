# Heading-Level `:ID:` Support — Implementation Plan

## Problem

Org-roam allows `:PROPERTIES:` drawers with `:ID:` not only at the note level
but under any heading:

```org
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: HTTP

* Versions
** HTTP/1
** HTTP/2
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
** HTTP/3
```

These heading-level IDs enable direct links to specific sections within a note
(`[[id:bbbb...][HTTP > HTTP/2]]`). Currently, `pkms` partially handles them
(links resolve, `check`/`validate` accept them), but cannot create, discover,
or report them as first-class entities. Skills (`note-writer`, `pkms-manager`)
have no guidance for heading-level IDs.

---

## Database Findings

6 heading-level `:ID:` properties across 5 notes in the user's database:

| Heading UUID | Note | Heading |
|---|---|---|
| `ba20ac20-...` | `roam/common/...-http.org` | `** HTTP/2` |
| `b2995a19-...` | `roam/common/...-lte.org` | `** Wireless and Cellular Communications` (duplicate of note UUID — bug) |
| `a0721cd8-...` | `roam/common/...-prometheus_node_exporter.org` | `** To start only after certain interface becomes active` |
| `b355cbbc-...` | `roam/...-example_note_with_code.org` | `* Link to the code` |
| `eb155a82-...` | `org-mode-good-text.org` | `* Time Clocking` |
| `f1d38e9a-...` | `org-mode-good-text.org` | `** Attachments` |

One cross-note link targets a heading UUID:
`systemd network online target` → `prometheus node exporter > To start only after...`


## Part 1: Code Changes (`src/`)

### 1.1 Parser — Assign UUIDs to Headings

**File**: `src/parser.rs`

**Current state**: `parse_note()` collects all `:ID:` values into a flat
`uuids: Vec<String>`. The `Heading` struct has no `uuid` field. Property drawers
are only parsed for ID values; heading context is not tracked.

**Change**:
1. Add `uuid: Option<String>` to `Heading` struct.
2. Track which heading the parser is currently "inside" during line-by-line
   scan. When `:ID:` is found in a PROPERTIES drawer that appears after a
   heading line but before the next heading, assign it to the most recent
   heading's `uuid` field.
3. The first `:ID:` before any heading remains the primary note UUID. Heading
   UUIDs should NOT be added to the flat `uuids` list (to avoid duplicate
   detection false positives). Store them only on the heading struct.
4. Add `heading_uuids() -> Vec<String>` helper method on `ParsedNote` that
   returns all non-empty heading UUIDs.

**Risk**: PROPERTIES drawers at the note level vs heading level are
distinguished by position relative to the first heading line. This heuristic is
fragile — notes can have multiple top-level PROPERTIES drawers (e.g., one for
`:ID:` and another for `:ROAM_ALIASES:`). Use the rule: *a PROPERTIES drawer
belongs to the most recent heading if one has been seen; otherwise it belongs
to the note.*

**Lines affected**: ~30 lines changed (Heading struct + parser state + UUID
assignment logic).

---

### 1.2 Graph — Register Heading UUIDs as Valid Link Targets

**File**: `src/graph/mod.rs`, `src/graph/builder.rs`

**Current state**: `Graph::build()` inserts every UUID from `parsed.uuids` into
`nodes` mapping them to the same `Node` clone. This means heading UUIDs already
work as link targets (they resolve to the parent note). However, the duplicate
detection logic treats all UUIDs equally — a heading UUID that matches another
note's primary UUID would reject the file as a duplicate.

**Change**:
1. **`Node` struct** (`src/graph/mod.rs:14`): Add `heading_uuids: Vec<String>`
   field populated from `parsed.heading_uuids()`.
2. **`Graph` struct** (`src/graph/mod.rs:64`): Add
   `heading_uuid_to_primary: HashMap<String, String>` — maps each heading UUID
   to the parent note's primary UUID.
3. **`Graph::build()`** (`src/graph/builder.rs:39`): 
   - When inserting nodes, only use `parsed.uuids[0]` (primary UUID) for
     `seen_uuids` duplicate detection, not heading UUIDs.
   - Populate `heading_uuid_to_primary` for each heading UUID pointing to
     `primary_uuid`.
   - Add heading UUIDs to `nodes` for link resolution (keep existing behavior).
4. **`Graph::find_node()`** (`src/graph/mod.rs:111`): Add fallback check:
   ```rust
   if let Some(primary) = self.heading_uuid_to_primary.get(target) {
       return self.nodes.get(primary);
   }
   ```
5. **`Node::from_parsed()`** (`src/graph/mod.rs:28`): Accept and store
   `heading_uuids`.

**Lines affected**: ~50 lines changed across 2 files.

---

### 1.3 `pkms new --heading` — Generate Heading-Level ID

**File**: `src/commands/new.rs`, `src/cli.rs`

**Current state**: `pkms new` generates one UUID for the note boilerplate.
No heading-level ID support.

**Change**:
1. **CLI** (`src/cli.rs:151-161`): Add `--heading` flag to `New` subcommand:
   ```rust
   #[arg(long, help = "Heading title to generate :ID: for")]
   heading: Option<String>,
   ```
2. **`NewOutput`** (`src/commands/new.rs:8`): Add
   `heading: Vec<HeadingId>` field:
   ```rust
   #[derive(Serialize)]
   pub struct HeadingId {
       pub title: String,
       pub uuid: String,
   }
   ```
   
**IMPORTANT**: This command should generate UUID for the specified heading
already presented in the file. If there is no such heading then error should be
returned. `pkms new --heading "This is title"` as usual new command just does
dry run. `pkms new --heading "This is title" --create` inserts properties drawer
for this heading with generated UUID.
   
3. **Output**: JSON includes `heading: {"title": "This is title", "uuid": "..."}`.

**Usage**:
```bash
pkms new "HTTP Overview" --create --heading "HTTP/1"
```

If note is not yet created, then error is returned.
If note is available and there is no such heading in this note, then error is returned.
If note is available and there is such heading in this note, then drawer is inserted.
If note is available and there are more than one such headings, then error is returned.

**Lines affected**: ~60 lines changed in `new.rs` + 5 in `cli.rs`.

---

### 1.4 `pkms resolve` — Find Notes by Heading UUID

**File**: `src/commands/resolve.rs`

**Current state**: `resolve` reads only the first 100 lines of each file
(header-only scan for speed). Heading `:ID:` properties deeper in the file are
invisible. `pkms resolve --uuid "a0721cd8"` returns 0 results.

**Change**:
**Full file scan** (simple, more thorough): When `--uuid` is
provided, scan the full file content using a regex for all `:ID:` matches
(not just the first one), collecting heading UUIDs alongside the primary UUID.
Return the note entry with its primary UUID but indicate that the match was via
a heading UUID.

1. Read full file content (not just 100 lines).
2. Use `UUID_RE.captures_iter()` to find ALL `:ID:` values in the file.
3. The first UUID is the primary. If the query matches *any* UUID, include
   the note. Add a `matched_heading_uuid: Option<String>` field to
   `ResolvedNote` to indicate when the match came from a heading UUID.
4. Add `--uuid` match against heading UUIDs specifically (not just primary).

**Lines affected**: ~40 lines changed.

---

### 1.5 `pkms get --headings` — Include Heading UUIDs in Output

**File**: `src/commands/get.rs`

**Current state**: `HeadingJson` has `level`, `title`, `todo_state`, `tags`,
`raw` — no `uuid`. `parse_headings_from_content()` does line-by-line regex
matching and cannot see multi-line PROPERTIES drawers.

**Change**:
1. **`HeadingJson`** (`src/commands/get.rs:12`): Add `uuid: Option<String>`.
2. **`parse_headings_from_content()`**: Replace the simple line-by-line
   regex approach with a stateful parser that tracks PROPERTIES drawers
   and assigns IDs to headings, similar to the main parser in `src/parser.rs`
   but outputting `HeadingJson` structs.
3. **Text output**: When a heading has a UUID, show it in the headings list:
   ```
   --- Headings ---
   * Introduction  :tag1:
   ** HTTP/1
   ** HTTP/2 (ba20ac20-4016-410e-9a63-9986968f4d4a)
   ** HTTP/3
   --- End Headings ---
   ```
4. **JSON output**: Include `"uuid": "ba20ac20-..."` in heading objects.

**Lines affected**: ~50 lines changed (heading parsing rewrite).

---

### 1.6 `pkms check` — UUIDs Duplicates

**File**: `src/graph/builder.rs`

**Change**: All UUIDs in the database should be unique without distinguishing
between headings and notes. UUID plays role of link target and should resolve
unambiguously.

---

### 1.7 `pkms validate` — Report Heading UUIDs

**File**: `src/commands/validate.rs`

**Change**: Uniqeness of all UUIDs in the file should be performed. Add
`heading_uuids: Vec<String>` field listing all heading-level UUIDs found in the
note. This helps the AI agent verify heading IDs exist.

---

## Part 2: Skill Changes (`skills/`)

### 2.1 `pkms-manager` SKILL.md — Heading Workflows

**File**: `skills/pkms-manager/SKILL.md`

**Changes**:

1. **Database section** (line 16): Add sentence about heading-level IDs:
   > Headings within notes may also carry `:ID:` properties for direct
   > section-level linking. Use `pkms get <target> --headings` to see heading
   > UUIDs, and `pkms new --heading` to create them.

2. **`new` command table** (line 158-165): Add:
   ```
   | `pkms new "Title" --create --heading "Heading 1"` | Fill UUID for already available "Heading 1" in the note |
   ```

3. **`resolve` command table** (line 77-86): Add:
   ```
   | `pkms resolve --uuid <uuid>` | Find a note by any :ID: (note-level or heading-level) |
   ```

4. **New workflow section**: "Creating and Linking Heading-Level IDs":
   ```
   ### Creating and Linking Heading-Level IDs

   When a topic within a note deserves its own anchor point for cross-linking:

   1. Create the note with heading (if note is not available):
      `pkms new "Note name" --create`
   2. Create necessary heading on the corresponding level manually (if heading is not yet available).
      `pkms new "Note name" --create --heading "Heading"`
   3. Find heading UUIDs in an existing note or get heading UUID from previous `new` command output:
      `pkms get <target> --headings --no-content --output-format json`
   4. Link to a specific heading from another note:
      `[[id:<heading-uuid>][contextual text here]]`
   5. Verify both notes with `pkms verify`.
   ```

---

### 2.2 `pkms-manager` References — Per-Command Updates

**File**: `skills/pkms-manager/references/new.md`

- Add `--heading` flag documentation (line 18-25).
- Add heading boilerplate format example (after line 39).
- Add scenario: "Create note with heading IDs for section linking" (after line 89).

**File**: `skills/pkms-manager/references/get.md`

- Update heading JSON example (line 80-82) to include `"uuid"`:
- Update text heading example to show UUID when present.

**File**: `skills/pkms-manager/references/resolve.md`

- Add note that `--uuid` matches both note-level and heading-level IDs.
- Add example: `pkms resolve --uuid "ba20ac20"` finds the HTTP note via its
  HTTP/2 heading ID.

---

### 2.3 `pkms-manager` Schemas — JSON Schema Updates

**File**: `skills/pkms-manager/schemas/get.json`

- Add `"uuid": { "type": "string" }` to heading items (line 19-26).
- Add it to `required` array? No — keep it optional since most headings won't
  have IDs.

**File**: `skills/pkms-manager/schemas/new.json`

- Add `headings` array to output:
  ```json
  "headings": {
    "type": "array",
    "items": {
      "type": "object",
      "properties": {
        "title": { "type": "string" },
        "uuid": { "type": "string" }
      },
      "required": ["title", "uuid"]
    }
  }
  ```

**File**: `skills/pkms-manager/schemas/resolve.json`

- Add `"matched_heading_uuid": { "type": "string" }` to result items
  (for when match came from a heading UUID).

**File**: `skills/pkms-manager/schemas/validate.json`

- Add `"heading_uuids": { "type": "array", "items": { "type": "string" } }`.

---

### 2.4 `note-writer` SKILL.md — Heading ID Writing Patterns

**File**: `skills/note-writer/SKILL.md`

**Changes**:

1. **New section**: "Heading-Level `:ID:` Properties" (after Heading Levels,
   around line 55):
   ```
   ### Heading-Level `:ID:` Properties

   When a heading covers a concept that deserves its own link anchor, add a
   PROPERTIES drawer with `:ID:` immediately after the heading line:

   ```
   ** HTTP/2
   :PROPERTIES:
   :ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
   :END:
   ```

   Generate heading UUIDs with `pkms new "Note Title" --create --heading "HTTP/2"`.
   Link to a heading from another note with:

   ```
   [[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][HTTP/2]]
   ```

   The link description should be used depending on the context of the surrounding text.

   Heading IDs are warranted when:
   - The heading covers a sub-concept that other notes need to reference directly.
   - The heading is a version/variant of the main topic (e.g., HTTP/2 under HTTP).
   - The heading is a distinct protocol, specification, standard, or case within a
     broader note.

   Do not add heading IDs mechanically to every heading. Only add them when
   cross-note linking to that specific section is expected.
   ```

---

## Part 3: Integration Tests

**File**: `tests/integration.rs`

Add test cases:

1. `test_new_with_heading` — verify `pkms new "Title" --create --heading "H1"`
   generates correct boilerplate for heading drawer.

2. `test_resolve_heading_uuid` — verify `pkms resolve --uuid "<heading-uuid>"`
   finds the parent note.

3. `test_get_headings_with_uuids` — verify `pkms get --headings --output-format json`
   includes `uuid` field in heading objects.

4. `test_link_to_heading_resolves` — create a note linking to a heading UUID,
   verify `pkms check` and `pkms validate` report the link as healthy.

5. `test_heading_uuid_duplicate_detection` — create two notes with clashing
   heading UUIDs, verify `pkms check` reports the duplicate.

---

## Part 4: Implementation Order

| Step | Component | Effort | Dependencies |
|---|---|---|---|
| 1 | Parser: Heading UUID extraction | Medium | None |
| 2 | Graph: Register heading UUIDs, dedup logic | Small | Step 1 |
| 3 | `get --headings`: Include UUIDs in output | Medium | Step 1 |
| 4 | `new --heading`: Generate heading IDs | Medium | None |
| 5 | `resolve --uuid`: Full-file UUID scan | Medium | Step 1 |
| 6 | `check`/`validate`: Heading UUID reporting | Small | Step 2 |
| 7 | Skills: Update all docs and schemas | Medium | Steps 1-6 |
| 8 | Integration tests | Medium | Steps 1-7 |

Steps 1-3 can be parallelized with step 4. Step 5 can start after step 1.
Skills (step 7) should be done last to reflect the final API.

---

## Part 5: Backward Compatibility

- All existing links continue to work — heading UUIDs already resolve to
  parent notes via the `nodes` map.
- `pkms check` behavior: duplicated uuids now will be reported.
- Existing notes with heading-level IDs are unaffected by parser changes —
  heading UUIDs are still collected and mapped.
- `pkms resolve --uuid` becomes slightly slower because
  it reads full files instead of first 100 lines. It is acceptable.

---

## Part 6: More Considerations

1. **Heading-level backlinks** — `pkms check` could report which headings
   receive incoming links, not just which notes.
2. **`pkms fix` for heading UUIDs** — the `fix` command already does
   regex-based replacement on full file content, so it should handle heading
   UUIDs without changes. Verify.
3. **`pkms suggest` for headings** — could suggest related notes to a specific
   heading within a note.
