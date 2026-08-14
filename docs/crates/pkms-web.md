# Crate: pkms-web

`crates/pkms-web` owns the local note viewer used by `pkms serve`. It renders
one selected note plus linked-note navigation through a foreground HTTP server.

## Responsibilities

- Load the current graph at server startup through `pkms-org`.
- Resolve the initial Org target from UUID, path, or title, or a standalone Org
  or Markdown target from any readable file path.
- Render org content to HTML, including headings, lists, tables, planning
  badges, tags, source blocks, inline formatting, formulas, and links.
- Render Markdown through `pulldown-cmark`, escape raw HTML, and omit graph-only
  backlinks and preview UI.
- Render Org files outside the graph with the normal Org renderer while omitting
  backlinks, UUID/tag metadata, and preview UI.
- Serve internal note navigation, note previews, local declared file links,
  declared attachment links, static assets, fonts, CSS, and JavaScript.
- Provide the "Open in Emacs" route through a callback supplied by the umbrella
  crate.
- Keep the viewer foreground-only and free of persistent derived state.

## Main Modules

| Module | Purpose |
|--------|---------|
| `lib.rs` | Narrow public viewer config, request, startup, and serving facade. |
| `serve/mod.rs` | Private viewer module wiring and shared page assets. |
| `serve/http.rs` | HTTP request routing and response handling. |
| `serve/page.rs` | Page shell, panels, backlinks, preview hooks, and note layout. |
| `serve/markdown_html.rs` | Markdown parsing, safe HTML events, and heading anchors. |
| `serve/org_html.rs` | Org-to-HTML rendering entry point. |
| `serve/org_html/blocks.rs` | Block-level org rendering. |
| `serve/org_html/lists.rs` | List rendering. |
| `serve/inline.rs` | Inline links, text formatting, math, and encoding helpers. |
| `serve/highlight.rs` | Syntect source highlighting and CSS. |
| `serve/assets.rs` | Font and static asset serving. |
| `serve/page.js` | Browser interactions for panels, previews, and open actions. |
| `serve.css` | Viewer styling and bundled font declarations. |

## Invariants

- `pkms serve` must be compiled only when the umbrella crate enables the
  `web` feature.
- The server is an explicit foreground command. Do not add watch mode,
  background daemon behavior, or persistent derived state.
- Local `file:` and `attachment:` targets are served only when the rendered
  note declares the exact link.
- File targets must resolve under the database root; attachment targets must
  resolve under supported org-attach roots.
- Internal `id:` links navigate through the viewer instead of exposing raw
  filesystem paths.
- External files are served only when they match the canonical startup target;
  arbitrary external `file=` query paths remain unavailable.

## Boundaries

`pkms-web` may depend on `pkms-org`. It must not depend on `pkms`,
`pkms-db`, `pkms-rag`, or `pkms-task`. CLI parsing and the editor-opening
callback live in the umbrella crate.

## Related Docs

- [Web Viewer](../web.md)
- [Command Reference](../commands.md#navigation)
- [Architecture](../architecture.md)
- [Development](../development.md#feature-specific-checks)
