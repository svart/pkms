# Installation

## Cargo

Install from a local checkout:

```bash
cargo install --path .
```

Build with Todoist task provider support:

```bash
cargo install --path . --features todoist
```

Build with the local web viewer:

```bash
cargo install --path . --features web
```

Build with SSH-backed remote file-link checks:

```bash
cargo install --path . --features ssh
```

Features can be combined when needed:

```bash
cargo install --path . --features todoist,web,ssh
```

Run directly from the checkout:

```bash
cargo run -- <args>
```

`pkms rag` is included in normal builds. The RAG crate uses FastEmbed by
default, so first indexing or dense retrieval may download model files. Use
`PKMS_RAG_EMBEDDING_PROVIDER=hash` for deterministic local runs that avoid
FastEmbed model setup.

## Todoist Feature

The `todoist` feature enables Todoist-backed task reads and writes through the
`task` namespace. Default builds reject `source:todoist` and `source:all`
commands that need Todoist data. Configure a token with `TODOIST_API_TOKEN` or
`[todoist].token`; see [Configuration](configuration.md).

## Web Feature

The `web` feature enables `serve`. Default builds do not expose that command.
The web viewer renders org notes as static HTML and uses KaTeX for formulas and
Syntect for source block highlighting.

## SSH Feature

The `ssh` feature enables explicit remote `file:` link checks for TRAMP-style
SSH targets such as `/ssh:host:/absolute/path`. Default builds keep
`--remote-file-links` visible but reject it with setup guidance.
