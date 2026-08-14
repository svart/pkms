# Web Viewer

`pkms serve` renders one note and linked notes in a local browser-friendly HTTP
viewer. It is built only when the umbrella crate enables the `web` feature.

## Build and Run

Run from a checkout:

```bash
cargo run --features web -- --db ~/Documents/org serve "Project Alpha"
cargo run --features web -- --db ~/Documents/org serve docs/project-alpha.md
```

Or install a binary with the web feature:

```bash
cargo install --path crates/pkms --features web
pkms serve "Project Alpha"
```

Bind settings:

```bash
pkms serve <uuid-or-title> --host 127.0.0.1 --port 8765
pkms serve <uuid-or-title> --port 0
```

`--port 0` asks the OS for a free port. On startup, text output prints the URL.
JSON output prints the same startup event in structured form:

```bash
pkms serve <target> --output-format json
```

The process stays in the foreground until interrupted.

## What It Renders

- The selected Org note or Markdown file body.
- Internal `id:` links as viewer navigation.
- Backlinks and table of contents panels.
- Hover previews for internal note links.
- Org tables, lists, tags, TODO states, priorities, planning badges, inline
  markup, source blocks, syntax highlighting, and static KaTeX formula HTML.
- Free-standing `http://` and `https://` URLs after org links are resolved.
- An "Open in Emacs" action that uses the same default editor path as
  `task open`.

Standalone targets may use an `.org`, `.md`, or `.markdown` extension and do
not need to be under the database root. Markdown uses CommonMark plus tables,
footnotes, strikethrough, and task lists; raw HTML is shown as text rather than
inserted into the page. Standalone files keep the table of contents and "Open
in Emacs" action, but omit backlinks, UUID and tag metadata, and note previews.
Opening a standalone file in Emacs always starts at line 1.

Org files represented in the configured database graph retain the complete Org
note view, including backlinks. An Org file outside that graph is rendered as a
standalone file. For safety, an external file is exposed only when it is the
server's explicit startup target; changing the URL to another external path
does not expose that file.

## Local File and Attachment Links

For graph-backed Org notes, the viewer serves local `file:` and `attachment:`
targets only when the note declares the exact link:

- `file:` targets must resolve under the database root.
- `attachment:` targets must resolve under supported org-attach roots.
- Undeclared local files are not exposed by the server.

This keeps browser access scoped to the note being rendered and the links that
note actually contains.

## Architecture Notes

`pkms serve` is a foreground local viewer, not a daemon or watcher. It loads the
graph at startup and keeps no persistent derived state.

CLI parsing and the editor callback live in the umbrella `pkms` crate. HTTP
routing, note rendering, static assets, previews, KaTeX, Syntect, and page
assets live in `pkms-web`.

## Related Docs

- [Command Reference](commands.md#navigation)
- [pkms-web crate](crates/pkms-web.md)
- [Architecture](architecture.md)
