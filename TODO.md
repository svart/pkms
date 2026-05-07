# pkms — Development Roadmap & Improvement Plan

## Priority Legend

- 🔴 **Critical** — bug or broken user-facing behavior
- 🟡 **Important** — material quality/structure issue
- 🔵 **Future** — new feature / strategic target

---

## 🔴 Critical Issues

### 4. DIY mustache template in `context.rs` is fragile

The hand-rolled template engine at `src/commands/context.rs:178-216` has edge cases:
- Unclosed `{{#tag}}` leaves raw tags in output
- Values containing `{{/tag}}` literally trigger premature close-tag matching
- The `while let` loop has no upper bound guard

**Fix:** Replace with the `handlebars` crate, or at minimum add input validation (reject templates with unmatched conditionals).

---

## 🔵 Strategic Targets (AI Agent Usage)

### 17. JSON Schema for all command outputs

Every command supports JSON output, but agents must either hardcode the shape or read docs. A `--schema` flag (or `pkms schema <command>`) emitting JSON Schema via `schemars` would let agents validate and navigate outputs programmatically.

**Value:** Very high for agent reliability. ~200 lines of integration.

### 19. Customizable `context` template

The template is hardcoded in `src/commands/context.rs:10`. Different LLMs benefit from different formatting:
- Claude: XML tags
- GPT: markdown sections
- DeepSeek: plain text with clear delimiters

**Fix:** Add `--template` / `--template-file` flags. Must fix issue #4 (DIY template engine) first — switch to `handlebars` crate, then expose template customization.

### 20. Section-level content extraction

`get` returns the full note. An agent often wants a specific heading subtree (e.g. "Implementation Details" only). A `--heading "Section Name"` filter on `get` would save tokens and focus attention.

**Pre-requisite:** The parser already extracts headings and their levels. This is display-only work in `get.rs`.
