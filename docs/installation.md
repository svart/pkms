# Installation

## Cargo

Install from a local checkout:

```bash
cargo install --path .
```

Build with embedding-based semantic search support:

```bash
cargo install --path . --features embed
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

## Nix

Install from a local checkout:

```bash
nix profile install .#default
nix profile install .#embed
nix profile install .#web
```

Install directly from GitHub:

```bash
nix profile install github:svart/pkms
nix profile install github:svart/pkms#embed
nix profile install github:svart/pkms#web
```

The flake currently exposes `default`, `embed`, and `web` packages. For a
Todoist-enabled install, use Cargo from a checkout, or enter `nix develop` and
run Cargo with `--features todoist`.

Run or build without installing:

```bash
nix run . -- <args>
nix run .#embed -- <args>
nix run .#web -- <args>
nix build
nix build .#embed
nix build .#web
```

Enter the development shell:

```bash
nix develop
```

Uninstall from a Nix profile:

```bash
nix profile list | grep pkms
nix profile remove <index>
```

## Embedding Feature

The `embed` feature enables `--embed` for `query` and `suggest`. Default builds
do not expose those flags. Embedding mode uses `fastembed` and downloads the
model on first use to the local cache.

## Todoist Feature

The `todoist` feature enables Todoist-backed task reads and writes through the
`task` namespace. Default builds reject `source:todoist` and `source:all`
commands that need Todoist data. Configure a token with `TODOIST_API_TOKEN` or
`[todoist].token`; see [Configuration](configuration.md).

## Web Feature

The `web` feature enables `serve`. Default builds do not expose that command.
The web viewer renders org notes as static HTML and uses KaTeX for formulas and
Syntect for source block highlighting.
