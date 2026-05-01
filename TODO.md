# pkms — Implementation TODO

## Phase 1: Core Architecture ✅

- [x] **Project scaffold**
  - [x] Set up CLI argument parsing (`clap` crate)
  - [x] Define subcommands: `check`, `validate`, `new`, `get`, `query`, `info`, `init-config`
  - [x] Global `--db` option for org-roam database root (overrides config)
  - [x] Global `--json` flag for structured JSON output on all commands
  - [x] Global `--verbose` / `-v` flag for detailed output

- [x] **Configuration (`~/.config/pkms.toml`)**
  - [x] Read and parse TOML config file (`toml` crate)
  - [x] `db_root` — path to org-roam database root
  - [x] `new_notes_dir` — directory where new notes are created (relative to db_root or absolute)
  - [x] `ignore_patterns` — list of glob patterns to skip during file discovery
  - [x] Merge config with CLI flags (`--db` overrides `db_root`)
  - [x] Graceful error if neither config nor `--db` is provided
  - [x] `--init-config` flag to generate default config file

- [x] **`info` — show current configuration**
  - [x] Display resolved config: db_root, new_notes_dir, ignore_patterns
  - [x] Show whether values came from config file or CLI override
  - [x] `--json` support for machine-readable output
  - [x] Show resolved paths (absolute, symlink-resolved)

- [x] **org-roam file discovery**
  - [x] Recursively find all `.org` files in the database root
  - [x] Build a file index: path → (filename, mtime, size)
  - [x] Apply `ignore_patterns` from config (glob matching)
  - [x] Always skip hidden dirs (`.`-prefixed)

- [x] **org-file parser**
  - [x] Extract `:ID:` from top-level property drawer (UUID v4)
  - [x] Extract `#+title:` value
  - [x] Extract `#+filetags:` value
  - [x] Extract `:ROAM_ALIASES:` property
  - [x] Extract all `[[id:<uuid>][...]]` links (outgoing edges)
  - [x] Extract `[[file:...][...]]` links
  - [x] Extract `[[https?://...][...]]` URL links
  - [x] Extract `[[attachment:...][...]]` links
  - [x] Extract heading structure (levels, TODO states, tags)

- [x] **In-memory graph model**
  - [x] `Node` struct: uuid, title, path, filetags, aliases, refs, content hash, links
  - [x] `Link` enum: Internal(uuid), File(path), URL(string), Attachment(name)
  - [x] `Graph` struct: node map (uuid → Node), link index (uuid → [Link])
  - [x] Index reverse links (backlinks) for each node

## Phase 2: Validation & Health Check ✅

- [x] **`check` — full database health scan**
  - [x] Validate every file has valid `:ID:` property (UUID v4 format)
  - [x] Validate every file has `#+title:` property
  - [x] Find files with duplicate UUIDs across the database
  - [x] Find files with duplicate titles
  - [x] Validate all `[[id:<uuid>]]` links point to existing nodes
  - [x] Detect dangling links (UUID references to nowhere)
  - [x] Detect orphan notes (0 incoming links, 0 outgoing links)
  - [x] Detect files without `:ID:` that look like they should have one
  - [x] Report database statistics (total notes, links, orphans, etc.)
  - [x] Exit codes: 0 = healthy, 1 = issues found

- [x] **`validate` — single note health check**
  - [x] Accept UUID, file path, or note title as argument
  - [x] Validate the note's `:ID:` and `#+title:` presence and format
  - [x] Check all outgoing links (internal and external)
  - [x] Check file existence for `[[file:...]]` links
  - [x] List all incoming backlinks
  - [x] Report all issues found
  - [x] Show link health summary

## Phase 3: Note Retrieval & Navigation ✅

- [x] **`get` — retrieve note with graph traversal**
  - [x] Accept UUID, file path, or note title
  - [x] `--depth` / `-d` option (default 1): how many hops to traverse
  - [x] `--out` / `-o` flag: return full note content + frontmatter
  - [x] `--graph` flag: visual ASCII art of the note neighborhood
  - [x] Include backlinks and forward links at each depth level
  - [x] Output as structured data (JSON for AI consumption)

- [x] **`path` — shortest path between two notes**
  - [x] Accept two UUIDs, paths, or titles
  - [x] BFS shortest path through the directed graph
  - [x] Output chain of notes with hop count
  - [x] `--max-depth` to limit search

- [x] **`subgraph` — export a subgraph around a note**
  - [x] Accept root UUID + depth
  - [x] Output JSON with all nodes and edges in the subgraph
  - [x] Show stats: vertex count, edge count, average vertex order

## Phase 4: Search & Query ✅

- [x] **`query` — fuzzy search across notes**
  - [x] Search by note title (word parts matching via substring)
  - [x] Search by note content (word parts matching via substring)
  - [x] Search by filetags (`--tag` / `-t` filter)
  - [x] Search by `:ROAM_REFS:` values
  - [x] Combined queries (e.g. `query "rust" --tag book`)
  - [x] Output: list of matching notes with title, UUID, path, match context
  - [x] `--limit` option for result count
  - [x] `--json` output format

- [x] **`tags` — list and search tags**
  - [x] List all `#+filetags:` values across the database
  - [x] Count notes per tag
  - [x] `--tag <tag>` to list all notes with that tag

## Phase 5: Note Creation ✅

- [x] **`new` — generate new note identity**
  - [x] Generate UUID v4 for the new note
  - [x] Generate timestamped filename: `YYYYMMDDHHMMSS-slug.org`
  - [x] Derive slug from title (lowercase, replace spaces with `_` and special chars with `-`)
  - [x] Place new note in `new_notes_dir` from config (default: `roam/`)
  - [x] Output: uuid, suggested filename, full path
  - [x] `--dry-run` default (just print without creating)
  - [x] `--create` flag to actually write the file with boilerplate
  - [x] `--tags` option to add initial filetags
  - [x] `--aliases` option to add aliases

- [x] **`add-link` — add a link from one note to another**
  - [x] Accept source note + target note (UUID, path, or title)
  - [x] Append `[[id:<target-uuid>][target-title]]` to source note
  - [x] Verify both notes exist before modifying
  - [x] Custom link description with `--description`

## Phase 6: Database Introspection & Statistics

- [ ] **`stats` — database statistics**
  - [ ] Total number of notes
  - [ ] Total number of links (internal, file, URL)
  - [ ] Average links per note
  - [ ] Number of orphan notes
  - [ ] Number of broken links (dangling UUIDs)
  - [ ] Most-linked notes (hub nodes)
  - [ ] Database size on disk
  - [ ] Breakdown by directory (common, personal, biblio, etc.)
  - [ ] Notes added/updated in last N days

- [ ] **`orphans` — list orphan notes**
  - [ ] List notes with 0 incoming and 0 outgoing links

- [ ] **`broken` — list broken/dangling links**
  - [ ] Show each broken link with source note and target UUID

- [ ] **`hubs` — list most-connected notes**
  - [ ] Rank notes by degree (incoming + outgoing link count)

## Phase 8: AI Integration

- [ ] **`context` — build context window for AI**
  - [ ] Given a note and depth, build a formatted context string
  - [ ] Include note content and linked neighbors
  - [ ] Optimize for token budget (--max-tokens)

## Tooling & Infrastructure

- [ ] **Testing**
  - [ ] Unit tests for org-file parser
  - [ ] Unit tests for graph operations
  - [ ] Integration tests with a mock org-roam database
  - [ ] Property-based tests for UUID/filename generation

- [ ] **Performance**
  - [ ] Profile parser on the full database (750+ files)
  - [ ] Parallel file parsing with rayon or tokio
  - [ ] Incremental parsing (cache parsed files, re-parse only changed)
  - [ ] Lazy content loading for query/search

- [ ] **Documentation**
  - [ ] README.md with examples (this file)
  - [ ] man page or `--help` text
  - [ ] Shell completions (bash, zsh, fish)

- [ ] **Packaging**
  - [ ] GitHub Actions CI pipeline
  - [ ] Pre-built binaries for Linux, macOS
  - [ ] Homebrew formula (optional)

## Implementation Notes

- Language: Rust (existing `Cargo.toml` with `edition = "2024"`)
- Suggested crates:
  - `clap` — CLI argument parsing with derive macros
  - `serde` / `serde_json` — JSON serialization
  - `uuid` — UUID generation and validation
  - `regex` — org-mode link and metadata extraction
  - `glob` — recursive file discovery
  - `fuzzy-matcher` or `skim` — fuzzy search
  - `rayon` — parallel file processing
  - `notify` — file system watcher for `watch` subcommand
  - `petgraph` — graph algorithms (BFS, shortest path)
  - `chrono` — timestamps for file naming and stats
  - `slug` — title-to-slug conversion
  - `ignore` — `.gitignore`-aware file walking
  - `toml` — configuration file parsing
  - `dirs` — standard XDG config directory discovery
