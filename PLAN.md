# pkms AI Agent Efficiency Plan

## Completed Items

| Item | Status | Notes |
|------|--------|-------|
| **2c**. `get` — Add `--headings` flag | ✅ Done | New `--headings` flag added to CLI, get command, and main dispatch. Shows heading structure (level, title, todo_state, tags) in both text and JSON output. |
| **3a**. SKILL.md — Remove hardcoded paths | ✅ Done | Replaced `~/Documents/org` with "configured via `~/.config/pkms.toml` or `--db` flag". Removed file counts. Added `info` as first command recommendation. Synced to installed copy. |
| **3c**. Missing JSON schemas | ✅ Done | Created schemas for `validate`, `get`, `path`, `new`, `fix`, `orphans`, `info`. Synced to installed copy. Now 13/13 commands have formal JSON schemas. |

---

## Architectural Constraint

**pkms is read-only except for `fix`.** All other note changes must be done manually by humans who read and understand the content. The tool's role is **discovery, retrieval, and context assembly** — not automated mutation. Agents use pkms to gather information, then instruct users what to do.

---

## Phase 1: New Commands (Read-Only)

### 1a. `dossier` — Topic aggregation for AI context building

The most impactful missing command. When a user asks a high-level question, an agent needs to gather related content from multiple notes into one structured output — instead of running `query` → pipe to `get` → manually correlate across N tool calls.

```
pkms dossier "machine learning" --max-tokens 8000 --depth 1
pkms dossier --from-stdin --max-tokens 8000
```

**Behavior** (read-only, zero mutations):
1. Takes search terms OR UUIDs from stdin
2. For matching notes: fetches full content, runs `suggest`, fetches graph neighbors at depth N
3. Deduplicates content — same paragraphs appearing across multiple notes are referenced, not repeated
4. Returns structured JSON
5. Token budget with smart truncation — cuts at paragraph/section boundaries, not mid-word
6. Includes cross-reference section showing how notes interconnect

**JSON output shape:**
```json
{
  "topic": "machine learning",
  "estimated_tokens": 7200,
  "max_tokens": 8000,
  "notes_included": 5,
  "notes": [
    {
      "uuid": "...",
      "title": "...",
      "path": "...",
      "filetags": [...],
      "content": "...",
      "neighbors": {
        "outgoing": [{"uuid": "...", "title": "...", "path": "..."}],
        "incoming": [{"uuid": "...", "title": "...", "path": "..."}]
      },
      "suggestions": [
        {"uuid": "...", "title": "...", "score": 120.0, "reasons": [...]}
      ]
    }
  ],
  "cross_references": {
    "shared_backlinks": [...],
    "shared_outgoing": [...],
    "paths_between_notes": [...]
  }
}
```

**Why this matters:** This is the single command an agent needs for "build broad concepts to answer high-level questions." Currently achieving the same requires 3+ separate tool calls and manual result correlation.

---

### 1b. `headings` — Extract note heading structure

Fast structural overview without loading full content. Helps agents decide if a note is worth deep-reading before fetching its entire body.

```
pkms headings <target>
pkms headings --from-stdin --output-format json
```

Returns heading hierarchy: `level`, `title`, `line_number`.

**JSON output shape:**
```json
{
  "uuid": "...",
  "title": "...",
  "headings": [
    {"level": 1, "title": "Introduction", "line": 5},
    {"level": 2, "title": "Background", "line": 12},
    {"level": 1, "title": "Main Content", "line": 45}
  ],
  "headings_count": 3
}
```

---

### 1c. `excerpt` — Search-focused content snippets with context

Like `query --content` but returns longer context windows around matches — more useful for AI comprehension than single-line snippets.

```
pkms excerpt "specific phrase" --context-lines 5 --max-notes 10
```

**JSON output shape:**
```json
{
  "query": "specific phrase",
  "total_results": 3,
  "results": [
    {
      "uuid": "...",
      "title": "...",
      "excerpts": [
        {
          "line": 42,
          "context_before": ["line 37", "line 38", "line 39", "line 40", "line 41"],
          "match": "line 42 with specific phrase highlighted",
          "context_after": ["line 43", "line 44", "line 45", "line 46", "line 47"]
        }
      ]
    }
  ]
}
```

---

## Phase 2: Enhance Existing Commands (Read-Only)

### 2a. `context` — Better AI context windows

**Current limitations:**
- Neighbor content hardcoded at 200 chars (too little for meaningful AI comprehension)
- Token estimation is naive word-counting (not character-based)
- Template is hardcoded in source
- No way to include suggestions
- No content deduplication

**Enhancements:**

| Enhancement | Detail |
|---|---|
| **Configurable neighbor trim** | Add `--neighbor-trim N` flag (default: 500). Also settable in `~/.config/pkms.toml` `[context]` section |
| **Better token estimation** | Use `chars / 3.5` for mixed content instead of ASCII word counting. Minimum: `chars / 4` |
| **Smart truncation** | Stop at newline boundaries; if mid-paragraph, backtrack to last complete paragraph |
| **Deduplication** | If a neighbor's opening paragraph is identical to the main note's content (boilerplate references, shared quotes), skip it |
| **Include suggestions** | Add `--with-suggestions` flag to append `suggest` results to the context window |

---

### 2b. `query` — Add `--fast` flag for hybrid search

Currently `query` always loads the full graph (`scan()` + `build()`) which is wasteful for broad searches. Hybrid approach:

```
pkms query "distributed systems" --fast --content
```

**Internal flow:**
1. `scan()` headers only (like `resolve`) — fast
2. Filter by title/alias/tag match
3. `build()` reduced set (only matching files)
4. Full-content search on candidates only

Result: fast title-first filtering with full-content search quality.

---

### 2c. `get` — Add `--headings` flag ✅

Return heading structure alongside content for quick orientation without a separate command call.

```
pkms get "Note A" --headings
```

Adds `headings` array and `headings_count` to the `node` object in JSON output.

---

## Phase 3: Skill Documentation Overhaul

### 3a. Remove hardcoded assumptions ✅

Current SKILL.md problems:
- Hardcodes `~/Documents/org` as database path
- Hardcodes file counts ("681 files in roam/common/", "73 files in roam/personal/")
- These are specific to the author's database, wrong for anyone else

**Fix:** Replace with:
- "configured via `~/.config/pkms.toml` or `--db` flag"
- "varies by database — run `pkms info` to see configuration"
- Add `info` as the recommended **first command** any agent should run

---

### 3b. Replace narrative workflows with prescriptive protocols

Replace the current 4 narrative workflows with 5 prescriptive protocols:

#### Protocol 1: Quick Fact Lookup
**When:** User asks a specific, narrow, factual question.
**Sequence:**
```bash
# Step 1: Fast title lookup (header-only scan)
pkms resolve --title "term" --output-format json
# If found, Step 2: Get full content
pkms get "<uuid>" --output-format json
```
**Fallback:** If `resolve` returns empty, try `query "term" --title --output-format json`.
**Output to analyze:** For JSON: `.results[].uuid`, then `.node.content` from `get`.

#### Protocol 2: Concept Deep Dive
**When:** User asks a broad, high-level, conceptual question requiring multiple sources.
**Sequence:**
```bash
pkms dossier "topic" --max-tokens 8000 --depth 1 --output-format json
```
**Fallback:** If `dossier` not yet implemented:
```bash
pkms query "topic" --output-format ndjson | pkms get --links --output-format json
# Or for targeted approach:
pkms resolve --title "topic" --output-format ndjson | pkms get --links --output-format json
```
**Output to analyze:** `dossier.notes[].content`, `dossier.cross_references`.

#### Protocol 3: Connection Discovery
**When:** User asks how two topics/notes are related.
**Sequence:**
```bash
# Find the path between them
pkms path "Topic A" "Topic B" --output-format json
# Get suggestions for each to find thematic overlap
pkms suggest "<uuid-of-A>" --limit 5 --output-format json
pkms suggest "<uuid-of-B>" --limit 5 --output-format json
```
**Fallback:** If `path` returns `found: false`, run `query` for both terms and compare neighbor graphs manually.
**Output to analyze:** `path.path[]` for connection chain, `suggest.suggestions[]` for thematic overlap.

#### Protocol 4: Database Health Check + Fix
**When:** User has manually edited notes or wants to verify integrity.
**Sequence:**
```bash
pkms check --output-format json
# For each broken link:
pkms resolve --title "Original Concept" --output-format json  # Find replacement
pkms fix "<broken-uuid>" "<replacement-uuid>"                 # Dry-run first
pkms fix "<broken-uuid>" "<replacement-uuid>" --apply          # Apply if correct
pkms check --output-format json                                # Verify
```

#### Protocol 5: New Note Creation
**When:** User wants to create a note on a topic.
**Sequence:**
```bash
# Check if similar notes already exist
pkms resolve --title "Topic" --output-format json
# If none found, create the note
pkms new "Topic Title" --create --tags "relevant,tags" --output-format json
# Validate the new note
pkms validate "<new-uuid>" --output-format json
```
**Important:** After creation, instruct the user to manually add links via their editor. Use `pkms suggest "<new-uuid>"` to find good link candidates and tell the user what to link.

**What to tell the user after `new --create`:**
```
Note created: "Topic Title" (UUID: <uuid>, Path: <path>)

To link this into your graph, manually edit the note and add links like:
  [[id:<target-uuid>][description]]

Suggested notes to link to (via pkms suggest):
  1. Related Note A (score: 120.0) — shared tags
  2. Related Note B (score: 85.0) — content keywords

Link conventions:
  - Use full UUIDs (dashed format), not 8-char prefixes
  - Embed links inline in existing sentences when possible
  - Add backlinks sparingly — only when there's a genuine contextual reason
```

---

### 3c. Add missing JSON schemas ✅

Currently 6 of 14 commands have JSON schemas. Create schemas for the remaining 8:

| Command | Schema file | Priority | Status |
|---|---|---|---|---|
| `validate` | `schemas/validate.json` | High | ✅ Done |
| `get` | `schemas/get.json` | High | ✅ Done |
| `path` | `schemas/path.json` | High | ✅ Done |
| `new` | `schemas/new.json` | Medium | ✅ Done |
| `fix` | `schemas/fix.json` | Medium | ✅ Done |
| `orphans` | `schemas/orphans.json` | Medium | ✅ Done |
| `info` | `schemas/info.json` | Low | ✅ Done |
| `headings` | `schemas/headings.json` | After 1b | Pending |
| `excerpt` | `schemas/excerpt.json` | After 1c | Pending |
| `dossier` | `schemas/dossier.json` | After 1a | Pending |

Each schema must follow draft-07 format, matching the existing 6 schemas.

---

### 3d. Add piping decision tree

Replace the flat list of pipeline examples with a structured decision tree:

```
Q: What do you have?
├─ Search terms
│  ├─ Need fast title/alias/tag lookup?  → resolve --title / --uuid / --tags
│  ├─ Need content search?               → query --content
│  ├─ Need both title + content fast?    → query --fast --content
│  └─ Need topic aggregation (AI)?       → dossier
├─ UUIDs
│  ├─ Need full content?                 → get
│  ├─ Need connections/discovery?        → suggest | get --links
│  ├─ Need validation (after edits)?     → validate
│  ├─ Need AI context window?            → context
│  └─ Need heading structure overview?   → headings
├─ Two topics
│  └─ Find connection path?              → path
├─ Nothing (exploring database)
│  ├─ Discover available tags?           → stats --tags
│  ├─ Find most-connected hubs?          → stats --hubs
│  ├─ Find disconnected orphans?         → orphans
│  ├─ Check overall health?              → check
│  └─ See current configuration?         → info
└─ Need to create a note?
   └─ Generate new note?                 → new --create
```

---

### 3e. Agent Usage sections in reference files

For each command reference file, add an "Agent Usage" section with:

```markdown
## Agent Usage

- **Preferred output format**: `--output-format json` for structured consumption
- **Field guarantees**: `uuid` and `title` always present; `filetags` may be `[]`; `content` present only with `get`/`context`
- **On zero results**: Returns empty arrays, not errors — check `total_results` or `count` fields
- **On partial match**: `resolve` matches substrings; `query` scores all matches; check `.score` field for relevance
- **Schema reference**: `schemas/<command>.json`
- **Performance**: `resolve` is header-only (~10ms); `query` loads full graph (~50ms per 1000 notes); `get`/`context`/`suggest` also load full graph
- **Piping**: If command is a producer, NDJSON output emits one JSON object per line with a `uuid` field
```

---

## Priority Matrix

| Rank | Item | Type | Effort | Impact | Status |
|---|---|---|---|---|---|---|
| 1 | SKILL.md — remove hardcoded paths, add `info` as first step | Doc | Low | High | ✅ |
| 2 | SKILL.md — prescriptive protocols + decision tree | Doc | Medium | High | Pending |
| 3 | Add JSON schemas for all commands (8 new) | Doc | Low | Medium | ✅ |
| 4 | `dossier` command | Code | High | Very High | Pending |
| 5 | `context` — configurable neighbor trim, better tokens | Code | Medium | High | Pending |
| 6 | `query --fast` flag | Code | Medium | Medium | Pending |
| 7 | `headings` command | Code | Low-Medium | Medium | Pending |
| 8 | `excerpt` command | Code | Medium | Medium | Pending |
| 9 | Agent Usage sections in all reference files | Doc | Medium | Medium | Pending |
| 10 | `get --headings` flag | Code | Low | Low | ✅ |
| 11 | `context --with-suggestions`, dedup, smart truncation | Code | Medium | Medium | Pending |

---

## What Is Intentionally Excluded

These are architecturally inappropriate for pkms as they would mutate org files programmatically:

| Rejected Idea | Reason |
|---|---|
| `link` command | Would add `id:` links to org files without human review |
| `edit` command | Would modify tags, aliases, titles programmatically |
| `rm` / `delete` | Would delete notes |
| Bulk tagging/untagging | Would modify `#+filetags:` programmatically |
| Note content update | Would modify note body |

The agent's role for "populate notes database" is:
1. Create new notes via `new --create`
2. Use `suggest` to find link candidates
3. **Instruct the user** what links to add manually (per existing "Linking Orphans" workflow in SKILL.md)
4. Validate the result with `validate` and `check`

---

## Success Criteria

After implementation, an AI agent should be able to:

1. **Answer high-level questions:** Run `dossier "topic"` → receive structured, deduplicated multi-note context → synthesize answer
2. **Answer specific questions:** Run `resolve --title` → `get` → extract precise information
3. **Discover connections:** Run `path A B` + `suggest A` + `suggest B` → correlate findings
4. **Create notes on user request:** Run `new --create` → `suggest <uuid>` → tell user what links to add
5. **Verify health:** Run `check` → `fix` for broken UUIDs
6. **Navigate efficiently:** Use the decision tree to pick the right command for each task
7. **Parse output reliably:** Validate all JSON against formal schemas
