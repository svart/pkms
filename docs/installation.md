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

Features can be combined when needed:

```bash
cargo install --path . --features todoist,web
```

Run directly from the checkout:

```bash
cargo run -- <args>
```

## Todoist Feature

The `todoist` feature enables Todoist-backed task reads and writes through the
`task` namespace. Default builds reject `source:todoist` and `source:all`
commands that need Todoist data. Configure a token with `TODOIST_API_TOKEN` or
`[todoist].token`; see [Configuration](configuration.md).

## Web Feature

The `web` feature enables `serve`. Default builds do not expose that command.
The web viewer renders org notes as static HTML and uses KaTeX for formulas and
Syntect for source block highlighting.
