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

Run directly from the checkout:

```bash
cargo run -- <args>
```

## Nix

Install from a local checkout:

```bash
nix profile install .#default
nix profile install .#embed
```

Install directly from GitHub:

```bash
nix profile install github:svart/pkms
nix profile install github:svart/pkms#embed
```

Run or build without installing:

```bash
nix run . -- <args>
nix run .#embed -- <args>
nix build
nix build .#embed
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
