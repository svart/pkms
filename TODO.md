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

### 18. Accurate token counting via tiktoken-rs

The current `estimate_tokens` uses a naive word-count heuristic that can be 30%+ off for code-heavy notes. Integrating `tiktoken-rs` with `--encoding` flag (cl100k_base, o200k_base, etc.) would give exact token counts matching the target LLM.

**Value:** High for agents working within context limits. Critical for `context --max-tokens`.

### 19. Customizable `context` template

The template is hardcoded in `src/commands/context.rs:10`. Different LLMs benefit from different formatting:
- Claude: XML tags
- GPT: markdown sections
- DeepSeek: plain text with clear delimiters

**Fix:** Add `--template` / `--template-file` flags. Must fix issue #4 (DIY template engine) first — switch to `handlebars` crate, then expose template customization.

### 20. Section-level content extraction

`get` returns the full note. An agent often wants a specific heading subtree (e.g. "Implementation Details" only). A `--heading "Section Name"` filter on `get` would save tokens and focus attention.

**Pre-requisite:** The parser already extracts headings and their levels. This is display-only work in `get.rs`.

### 21. Embedding-based similarity for `suggest` and `query`

Current search is substring-only. For an agent looking for conceptually related notes (e.g., "exception safety" → "RAII", "error handling"), substring matching misses. A lightweight option using `fastembed` or a local embedding server to compute on-the-fly semantic similarity would dramatically improve relevance.

This is NOT RAG — it's a better similarity metric for the existing `suggest` and `query` commands, respecting the stateless model (compute fresh each run).

### 25. Agent self-discovery — `pkms info --capabilities`

An agent's first interaction should tell it what the tool can do. A `--capabilities` flag on `info` (or dedicated `capabilities` command) that returns a JSON object listing all commands, their flags, argument types, and output schema URLs would let agents bootstrap without reading SKILL.md each time.
