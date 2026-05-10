# Heading Nodes Refactoring Plan

## Motivation

Currently, heading UUIDs in the graph are stored as **clones** of the parent note's `Node` — same `uuid`, same `outgoing`, same `title`. This is a lie: a heading with `:ID:` is an independent entity in org-roam that can be linked to, extracted, and eventually promoted to its own file. The clone approach works for link resolution but breaks down for:

- **Overlinking detection** — cannot distinguish links "from heading" vs "from file"
- **Validation** — heading-level analysis is imprecise
- **Suggest scoring** — heading nodes don't have their own link graph
- **Backlinks** — link to heading UUID resolves to parent UUID, losing precision

## Design

### Core idea

Each heading with `:ID:` becomes a **first-class `Node`** in the graph, with:

- Its own `uuid` (= heading's `:ID:`)
- Its own `title` (= heading text)
- Its own `outgoing: Vec<Link>` (= links found under this heading subtree)
- An explicit parent→child edge: `Link::Internal(child_uuid)` added to parent's outgoing, `Link::Internal(parent_uuid)` added to child's outgoing
- `filetags`, `categories`, `aliases`, `refs` inherited from parent + heading-level overrides
- `headings_count`, `heading_uuids` for sub-headings
- `path` pointing to the same file (but the heading's context is within the file at a specific line)

### What does NOT change

- `Node` struct fields remain the same (no new fields needed)
- `Link` enum stays the same (`Internal`, `File`, `Url`, `Attachment`)
- `Graph` struct remains the same
- All output structs, JSON shapes, CLI flags — unchanged
- `find_node`, `resolve_target` — already work with heading UUIDs
- `backlinks` — already keyed by UUID, just more precise now
- `broken_links` — parent→child edges are valid internal links

### Nesting

Headings can nest arbitrarily. A heading under another heading UUID becomes a child of the nearest enclosing heading UUID node (not of the file-level primary). The tree structure is:

```
File UUID1                    ← primary node
├── Heading 1 (no UUID)       ← not a graph node
├── Heading 2 (UUID2)         ← graph node, parent=UUID1
│   ├── Heading 3 (no UUID)   ← not a graph node
│   └── Heading 4 (UUID3)     ← graph node, parent=UUID2
├── Heading 5 (no UUID)       ← not a graph node
├── Heading 6 (UUID4)         ← graph node, parent=UUID1
└── Heading 7 (no UUID)       ← not a graph node
```

Edges: UUID1↔UUID2, UUID2↔UUID3, UUID1↔UUID4

## Implementation steps

### Step 1: Parser — attribute links to headings

**File:** `src/parser.rs`

The parser needs to track which heading a link belongs to. Currently `ParsedNote` has a flat `outgoing: Vec<Link>`. The heading struct needs its own outgoing:

```rust
pub struct Heading {
    pub level: usize,
    pub title: String,
    pub todo_state: Option<String>,
    pub tags: Vec<String>,
    pub uuid: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub priority: Option<char>,
    pub outgoing: Vec<Link>,           // NEW — links under this heading
}

pub struct ParsedNote {
    pub uuids: Vec<String>,
    pub title: Option<String>,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub roam_aliases: Vec<String>,
    pub roam_refs: Vec<String>,
    pub outgoing: Vec<Link>,           // file-level links (outside any heading)
    pub headings: Vec<Heading>,
}
```

**Parsing logic change:** The `LINK_RE` matches need to be attributed to the currently-active heading stack. During parsing:
- When a heading is entered, push it onto the heading stack
- When a link is found, push it into `headings[stack.top()].outgoing` (or `parsed.outgoing` if no heading is active)
- When a heading exits, pop the stack

This requires the parser to maintain a heading stack during the line-by-line pass.

**Edge cases:**
- Links before any heading → file-level (`parsed.outgoing`)
- Links in a heading's content before its first sub-heading → that heading's `outgoing`
- Links in a sub-heading → the sub-heading's `outgoing`
- Links in properties drawers → file-level

---

### Step 2: Builder — create real nodes for heading UUIDs

**File:** `src/graph/builder.rs`

The builder receives `Vec<FileScanResult>` where each `ParsedNote` now has per-heading outgoing links.

**Algorithm (recursive):**

```
fn process_headings(
    headings: &[Heading],
    parent_uuid: &str,
    parent_node: &Node,
    path: &Path,
    inherited_filetags: &[String],
    inherited_categories: &[String],
    inherited_aliases: &[String],
    inherited_refs: &[String],
    heading_uuid_to_primary: &mut HashMap<String, String>,
    all_uuids_seen: &mut HashMap<String, PathBuf>,
    duplicate_uuids: &mut Vec<DuplicateEntry>,
    nodes: &mut HashMap<String, Node>,
    title_to_uuid: &mut HashMap<String, Vec<String>>,
    uuid_to_outgoing: &mut HashMap<String, Vec<Link>>,
):
    for heading in headings:
        if let Some(uuid) = &heading.uuid:
            child_node = Node {
                uuid: uuid.clone(),
                title: heading.title.clone(),
                path: path.clone(),
                filetags: inherited_filetags + heading.tags,
                categories: inherited_categories,
                aliases: inherited_aliases,
                refs: inherited_refs,
                outgoing: heading.outgoing.clone() + parent_link,    // parent_link = Internal(parent_uuid)
                headings_count: sub_heading_count,
                heading_uuids: sub_heading_uuids,
                has_todos: heading.todo_state.is_some(),
            }
            nodes.insert(uuid, child_node)
            uuid_to_outgoing.insert(uuid, heading.outgoing + parent_link)
            heading_uuid_to_primary.insert(uuid, primary_uuid)         // still map to file-level primary for resolution
            heading_uuid_to_primary.insert(parent_uuid, parent_uuid)   // parent is already primary or heading

            // Add parent→child edge to parent's outgoing
            parent_outgoing = uuid_to_outgoing.get_mut(parent_uuid)
            parent_outgoing.push(Link::Internal(uuid.clone()))

            // Recurse into sub-headings
            process_headings(sub_headings, uuid, ...)
```

**Key decisions:**

1. **Title:** Use heading text, not file title. This is a semantic improvement — `find_node` by heading title works.
2. **Parent→child edge direction:** Both ways — parent has `Internal(child)` and child has `Internal(parent)`. This makes graph algorithms (BFS, hubs, suggest) naturally account for parent-child relationships.
3. **Heading UUID → primary UUID mapping:** Keep `heading_uuid_to_primary` mapping heading UUIDs to the file-level primary UUID (or the nearest containing heading UUID). This is needed for commands like `suggest` that may want to group results by file.

---

### Step 3: Graph traversal — what changes

**File:** `src/graph/traversal.rs` (and other consumers)

`get_neighbors` already filters by `Link::Internal`, so parent→child edges appear as regular neighbors. **No change needed** — this is correct behavior.

`find_shortest_path` already works with any `Link::Internal` edge. A path might route through parent-child edges, which is semantically correct (the heading IS connected to its parent). **No change needed.**

---

### Step 4: Analytics — what changes

**File:** `src/graph/analytics.rs`

**`hubs()`:** Degree counts include parent-child edges. This is correct — heading nodes with many links (explicit + parent-child) ARE hubs in the knowledge graph. **No change needed.**

**`orphan_nodes()`:** A heading UUID with no explicit incoming/outgoing links (only parent-child edges) is NOT an orphan — it has incoming from parent and outgoing to parent. But it IS only connected to its family. This might be surprising. **Consider:** should `orphan_nodes` exclude heading UUIDs? Or should it count them as non-orphans because they have parent→child edges? I think they are non-orphans — they're meaningfully connected.

**`stats()`:** Total link counts include parent-child edges. Internal link counts go up. This is more accurate (those edges exist). **No change needed.**

---

### Step 5: Commands — what changes

#### `detect_self_links`
**File:** `src/graph/mod.rs`

**No false positives.** Parent UUID ≠ child UUID, so a parent→child link is not a self-link. ✓

#### `detect_overlinks`
**File:** `src/graph/mod.rs`

**Simplifies dramatically.** No more `seen_primaries` dedup. Each heading node has its own `outgoing` with only links from its own subtree. Rules 1-3 work naturally:

```rust
pub fn detect_overlinks(&self) -> Vec<OverlinkEntry> {
    let mut results = Vec::new();
    for node in self.nodes.values() {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for link in &node.outgoing {
            if let Link::Internal(target) = link {
                *counts.entry(target).or_default() += 1;
            }
        }
        for (target_uuid, count) in counts {
            if count >= 2 {
                let target_title = self.nodes.get(target_uuid)
                    .map(|n| n.title.as_str()).unwrap_or("<unknown>");
                results.push(OverlinkEntry {
                    source_uuid: node.uuid.clone(),
                    source_title: node.title.clone(),
                    target_uuid: target_uuid.to_string(),
                    target_title: target_title.to_string(),
                    count,
                });
            }
        }
    }
    results
}
```

#### `check`
**File:** `src/commands/check.rs`

- Cross-links and overlinking work per-node. **No changes.**
- File link checks iterate `graph.nodes.values()` — heading nodes appear. Each heading uses the same file path. For file-type link existence checks, a heading node's link checks a file on disk. **No change needed** — heading nodes with `path` pointing to the same file is fine.
- Duplicate UUID detection already handles heading UUIDs correctly in builder.

#### `validate`
**File:** `src/commands/validate.rs`

- `validate_one` gets a heading node — `outgoing` is per-heading, which means overlinking check per-heading ✓
- Self-link check: parent→child link is NOT a self-link (different UUIDs) ✓
- File-type link resolution: heading node has the same `path` as parent ✓

#### `suggest`
**File:** `src/commands/suggest.rs`

Heading nodes now have their own incoming/outgoing, making scoring per-heading accurate. **No code change** — the existing scoring logic (link intersection, tag overlap, category overlap, ref overlap) works on `Node.outgoing` which is now per-heading.

#### `get`
**File:** `src/commands/get.rs`

When fetching neighbors of a heading node, you see:
- Its explicit links (links under that heading subtree)
- Its parent link (if heading UUID)
- Its child links (if sub-headings have UUIDs)

This is more informative. **No code change needed.**

#### `path`
**File:** `src/commands/path.rs`

Shortest path might route through parent-child edges. Example: path from UUID1 to UUID4 might go UUID1 → UUID2 → UUID4 if UUID2 links to UUID4, or UUID1 → UUID4 directly if edges exist. This is correct graph behavior. **No code change needed.**

#### `query`, `resolve`, `orphans`, `stats`, `context`, `agenda`, `todo`

These work with `Node.uuid` and `Node.title` which are now correctly per-heading. **No code changes** (except `stats` which might need to exclude heading nodes from total_notes count — see below).

---

### Step 6: Count heading nodes separately

**File:** `src/graph/analytics.rs` (stats), `commands/stats.rs`

Currently `total_notes` counts unique files via `path_to_uuid.len()`. This should remain:

```rust
let total_notes = self.path_to_uuid.len();  // unique files only, unchanged
```

But we might want an additional `heading_count` in `GraphStats`:

```rust
pub struct GraphStats {
    // existing fields...
    pub total_nodes: usize,   // total nodes including heading UUIDs (previously same as total_notes if no heading UUIDs existed outside path_to_uuid)
}
```

Actually, `total_nodes` is just `self.nodes.len()` and isn't needed as a new stat field. The `nodes` HashMap now includes heading UUIDs, but `path_to_uuid` counts files. The main `stats` command should still say "Notes" = unique files.

---

### Step 7: Dedup in graph iteration

Several parts of the codebase iterate `self.nodes.values()` and process duplicate paths. With heading nodes, this becomes more common. Audit all node iteration:

- `check` broken file links — iterates nodes, filters by `Link::File`. Heading nodes produce the same file-level checks as parent. Could skip heading nodes or accept redundant checks. Redundant checks are harmless.
- `check` filetags — same path read multiple times. Use `path_to_uuid` for iteration instead.
- `stats` — uses `path_to_uuid` for file count, `primary_nodes()` for link counting. OK.
- `hubs` — uses `primary_nodes()`. **Need to update:** heading nodes should also be considered for hub scoring (they can be hubs too).

Actually, `primary_nodes()` filters by unique `node.uuid`. Since heading nodes have unique UUIDs, they will appear. If we want to exclude heading nodes from certain statistics, add a separate method `file_nodes()`.

---

### Step 8: Update AGENTS.md

Update documentation to reflect:
- New graph model: heading UUIDs are independent nodes with explicit parent-child edges
- `Node.outgoing` now contains per-heading-zone links
- `Graph.nodes` includes heading UUID nodes
- Overlinking rules (rules 1-3) work naturally

---

## Migration strategy

No data migration needed — the graph is rebuilt from scratch on every invocation (stateless model). The change is purely in-memory.

## Testing plan

### Unit tests

1. **Parser tests:** Verify link attribution — links before any heading go to `parsed.outgoing`, links under a heading go to `heading.outgoing`, nested headings attribute correctly
2. **Builder tests:** Verify parent UUID node has `Internal(child)` in outgoing, child node has `Internal(parent)` in outgoing, nested chain (grandparent→parent→child) works
3. **Overlinking tests:** Nothing changes — heading nodes with 1 link to X and the parent node with 1 link to X are separate nodes with count=1 each (no overlinking) ✓
4. **Self-link tests:** Parent→child edges should not be flagged as self-links ✓
5. **Traversal tests:** BFS should traverse through parent-child edges ✓

### Integration tests

Existing test files should pass mostly unchanged. The main differences:
- `check` output may show higher internal link counts (parent-child edges)
- `validate` output for a heading UUID node shows per-heading outgoing/incoming counts
- `stats` total links includes parent-child edges (slightly higher numbers)

Update snapshot tests in `tests/integration/snapshot.rs` if needed.

## Risk assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Link count changes in stats | Test assertions fail | Update test expectations (correctness improvement) |
| Orphan detection changes | Heading-only nodes now have parent edges | Add `file_nodes()` vs `all_nodes()`, document behavior |
| Parser performance regression | Per-heading link attribution | O(n) per link, same as current. No overhead. |
| Nested heading cycles | Infinite recursion during build | Builder processes nesting depth-first, no cycles possible |
