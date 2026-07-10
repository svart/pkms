# Refactoring Assessment and Plan

Date: 2026-07-10
Scope: current `v2.3.0` checkout (`094ab4d`)

## Executive summary

The workspace already has a good high-level shape: the executable crate depends
on focused domain crates, domain crates do not depend back on the executable,
optional integrations are feature-gated, and the normal runtime remains a
stateless scan/compute/print operation. The current all-feature Clippy gate and
the crate-boundary script both pass.

The main maintainability problem is not the dependency arrows; it is that some
responsibilities sit on the wrong side of otherwise-correct arrows:

1. Task policy is split between `pkms-org`, `pkms-task`, `pkms-db`, and `pkms`.
   A task change therefore requires understanding four crates even though the
   dependency graph contains no cycle.
2. `pkms-org` exposes a large mutable-looking graph data structure and also owns
   task policy and token counting. This makes the lowest-level crate a shared
   toolbox instead of a narrow org syntax/snapshot boundary.
3. Process concerns are only partly isolated. `pkms-db` launches editors,
   prints directly, and returns `std::process::ExitCode`; environment reads are
   correctly kept out of domain crates but remain spread across several
   umbrella modules.
4. `pkms-rag` exposes its SQLite implementation through public functions taking
   `rusqlite::Connection`, and its index state machine is encoded with strings.
5. Several broad public facades and large mixed-responsibility files hide dead
   code from the compiler and make local changes harder to review.

The recommended approach is incremental. First lock down observable CLI
contracts and strengthen boundary checks. Then correct task and process
ownership. After that, encapsulate the org snapshot and RAG store. File splits,
dependency pruning, and API narrowing should accompany those moves, not become
an independent rewrite.

## Assessment method

This review covered:

- workspace manifests and `scripts/check-crate-boundaries.sh`;
- architecture, development, and per-crate documentation;
- public module/facade surfaces;
- representative CLI adapters and domain command implementations;
- org parsing, graph/workspace loading, task extraction and mutation;
- RAG storage, indexing, retrieval, and HTTP routing;
- web viewer entry points and callbacks;
- configuration/environment resolution and output handling;
- file-size and dependency hotspots; and
- unit and integration test layout.

Baseline verification performed during the review:

```text
scripts/check-crate-boundaries.sh                         PASS
cargo clippy --workspace --all-targets --all-features \
  -- -D warnings                                         PASS
```

Passing these checks means the current tree is healthy by its existing gates.
It does not disprove the responsibility and encapsulation issues below; several
of them involve public items or allowed dependencies that Clippy cannot flag.

## What should be preserved

- The one-way workspace dependency direction.
- Stateless fresh scans for normal commands.
- Explicit foreground-only web and RAG servers.
- Stable text, JSON, and NDJSON behavior.
- Canonical, deterministic task IDs.
- Typed org edit requests for user-data mutations.
- Feature isolation for Todoist, SSH, web, and RAG.
- Focused command adapters rather than a generic command framework.

These are good constraints. The refactor should make the code match them more
directly rather than introduce service containers, persistent caches, daemons,
or a generic `core` dumping-ground crate.

## Target crate boundaries

| Crate | Target responsibility | Must not own |
|---|---|---|
| `pkms` | CLI parsing, process environment/config resolution, stdout/stderr, exit codes, editor launching, and cross-domain wiring | Org parsing, task policy, note algorithms, storage internals |
| `pkms-org` | Org discovery/parsing, immutable loaded snapshot, graph indexes/traversal primitives, link/path resolution, and raw typed org edits | Agenda policy, canonical task IDs, task filtering, tokenization, editor processes |
| `pkms-db` | Note-database use cases and checks over `pkms-org` | Task-only commands, process exit codes, direct printing, editor execution |
| `pkms-task` | All task semantics: state configuration, extraction/projection, canonical IDs, filters, providers, show details, and mutations | CLI/table formatting and raw process environment reads |
| `pkms-rag` | Retrieval records, indexing service, storage abstraction, embeddings, retrieval, and RAG HTTP API | CLI/environment precedence and direct coupling to the note viewer implementation |
| `pkms-web` | Viewer configuration, rendering, HTTP routes/assets, and typed open requests | Editor process execution and RAG types |
| new `pkms-tokens` | Token encoding and counting only | Org, task, command, or retrieval policy |

The new token crate is deliberately narrow. It is justified because
`tiktoken-rs` currently enters every `pkms-org` consumer, including standalone
`pkms-web`, although token counting is used only by `get` and RAG retrieval.

The intended dependency graph becomes:

```text
pkms
├── pkms-db ─────┐
├── pkms-task ───┤
├── pkms-rag ────┼──> pkms-org
├── pkms-web ────┘
└── pkms-tokens <──── pkms-db, pkms-rag
```

## Findings and recommendations

### 1. Task behavior crosses three domain boundaries

Priority: **highest**

Evidence:

- `crates/pkms-org/src/org_task_extract.rs` accepts valid/closed states and a
  clock, distinguishes TODO from agenda queries, and computes overdue state.
  Those are task policies, not raw org extraction.
- `crates/pkms-org/src/graph/tasks.rs` contains `TaskStateConfig`, although its
  only meaningful consumers are task behavior and web presentation.
- `crates/pkms-db/src/commands/show.rs` defines `RelatedTaskHeading`, accepts
  `TaskIdEntry`, and renders `p<ID>` parent/child chains. The command is only
  reached through `pkms task ... show`; there is no top-level note-database
  `show` command.
- `crates/pkms/src/commands/task/mod.rs` builds canonical task-ID entries and
  translates them into the `pkms-db` show model.

Why this is costly:

- A task semantics change can touch `pkms-org`, `pkms-task`, `pkms-db`, and the
  umbrella adapter.
- The lowest-level crate needs clock-relative concepts that are irrelevant to
  graph construction.
- `pkms-db` cannot use `pkms-task` types because of the dependency rule, so the
  umbrella crate performs avoidable model translation.
- Ownership is inferred from call paths rather than from directory/crate names.

Recommended fix:

1. Keep raw parsed headings, planning timestamps, priorities, tags, projects,
   and typed edit primitives in `pkms-org`.
2. Move `TaskStateConfig`, TODO/agenda inclusion rules, overdue calculation,
   and `OrgTaskRecord` projection into `pkms-task`.
3. Move the task-show execution/output model from `pkms-db` into `pkms-task`.
   It can consume `pkms-org` headings and the canonical task index directly.
4. Move `crates/pkms/src/commands/show.rs` under
   `crates/pkms/src/commands/task/show.rs` so the adapter location matches its
   only user-facing namespace.
5. Give `pkms-web::WebConfig` a small presentation-specific TODO-state class
   (`open_states`, `closed_states`) instead of importing a task config type.

Do not move raw org string editing into `pkms-task`. `pkms-task` should decide
*what* mutation is needed and send a typed request; `pkms-org` should continue
to own *how* that mutation is safely represented in an org file.

### 2. Process and presentation boundaries are inconsistent

Priority: **high**

Evidence:

- `crates/pkms-db/src/commands/open.rs` resolves a target, prints to stdout and
  stderr, parses an editor command, and starts a child process.
- `crates/pkms-db/src/commands/check.rs` and its output model carry
  `std::process::ExitCode` into a domain crate.
- `crates/pkms-db/src/commands/resolve.rs` returns selected output fields as part
  of `ResolveCommandOutput`, then filters `serde_json::Value`; this is output
  projection rather than database execution.
- `crates/pkms/src/runner.rs` contains the `init-config` command implementation
  in addition to dispatch.

Why this is costly:

- Domain functions are harder to reuse and test without capturing process I/O.
- Exit semantics and editor behavior can change for reasons unrelated to note
  database logic.
- Output-only options travel into and back out of domain execution.
- The documented statement that the umbrella owns process/output behavior has
  exceptions that future code is likely to copy.

Recommended rule:

- Domain crates may return serializable output models and may temporarily keep
  pure `render_text(&Output) -> String` helpers close to those models.
- Only `pkms` may select an output format, write stdout/stderr, return an
  `ExitCode`, inspect terminal state, or launch an editor.
- HTTP responses remain owned by the server crates because HTTP is their public
  interface, not CLI process output.

Concrete changes:

- Replace `pkms-db::open::execute` with a pure target-resolution operation that
  returns a typed path/line/title. Put editor command parsing and process launch
  in an umbrella `editor` module.
- Let `CheckCommandOutput` contain `healthy: bool`; map that to `ExitCode` in
  `commands/check.rs`.
- Keep `ResolveOptions` limited to search behavior. Apply `--fields` in the
  umbrella render adapter using a typed projection or a single centralized
  structured-output helper.
- Move `init_config` to `commands/init_config.rs`; keep `runner.rs` as dispatch.

### 3. `pkms-org` is both a snapshot model and a shared toolbox

Priority: **high**

Evidence:

- `Graph` exposes `nodes`, lookup indexes, backlinks, diagnostics, heading
  indexes, and scan results as public fields in
  `crates/pkms-org/src/graph/mod.rs`.
- Other crates directly access `graph.nodes`, `graph.backlinks`,
  `graph.heading_uuid_to_primary`, and `graph.results`.
- `Graph` retains scan results, while `Workspace` owns a `Corpus` and another
  graph built from cloned parsed results. Several commands load a graph and
  then read the same source file again.
- `OrgConfig` mixes scan inputs (`db_root`, ignore patterns), link resolution
  (`home_dir`), and note-creation directories that graph loading does not use.
- `pkms-org::tokens` brings `tiktoken-rs` into every crate that depends on
  `pkms-org`.

Why this is costly:

- Public fields make graph invariants unenforceable and turn internal indexes
  into de facto APIs.
- Callers must know whether to use `Graph`, `Corpus`, `Workspace`, or a second
  filesystem read.
- Broad configuration makes low-level APIs accept data they do not need.
- A heavy unrelated dependency increases compile surface for task and web
  consumers.

Recommended fix:

1. Introduce one clearly named loaded value, preferably `OrgSnapshot`, which
   owns parsed files and graph indexes from one scan. Keep `Graph::from_*` as a
   pure construction detail.
2. Add narrow query methods (`node`, `nodes`, `backlinks`, `scan_results`,
   `contains_path`, `heading_owner`) and migrate callers in small batches.
   Privatize fields only after all consumers use those methods.
3. Split configuration by use:
   - `ScanConfig { db_root, ignore_patterns }`;
   - `LinkResolutionContext { db_root, home_dir }`; and
   - note/daily creation paths owned by the relevant command/task config.
4. Let read-only commands that need source content read it from the same
   snapshot. Commands that intentionally need current-on-disk content should
   make that second read explicit in the function name.
5. Move token encoding/counting out of `pkms-org`.

This is an encapsulation refactor, not authorization for persistent caching.
`OrgSnapshot::load` should still scan and parse fresh on every normal command.

### 4. Environment resolution is in the right crate but remains scattered

Priority: **medium-high**

Recent work correctly removed process environment reads from domain crates.
That direction should be preserved. The remaining issue is testability and a
partially resolved umbrella configuration:

- `Config::load` chooses the platform config directory internally.
- `ResolvedConfig::org_config`, `resolved_info`, Todoist helpers, and
  `runner::init_config` call `dirs` or read environment values at different
  times.
- `commands/check.rs` resolves SSH user, agent socket, home directory, known
  hosts, and identity files.
- `commands/rag.rs` independently resolves four RAG environment variables and
  its tests mutate global environment variables.
- `test_config_load_ok` reads the real user's config location, so it is not a
  hermetic unit test.

Recommended fix:

- Add a small umbrella-only `ProcessContext`/`Environment` value captured at
  startup: config path, home directory, current directory, selected environment
  variables, and terminal capabilities.
- Change `Config::load` to `Config::load_from(path)` and make precedence
  resolution accept explicit process inputs.
- Keep command-specific validation lazy: an unrelated command must not fail
  because a Todoist token or FastEmbed configuration is absent.
- Build typed `SshFileCheckOptions`, `TodoistProviderConfig`, `RagConfig`, and
  `WebConfig` from explicit values at the adapter boundary.
- Replace environment-mutating tests with table-driven tests over an injected
  environment map.

The goal is deterministic construction, not a dependency-injection framework.
A plain struct plus pure conversion functions is enough.

### 5. The RAG facade leaks storage and uses a stringly typed state machine

Priority: **medium-high**

Evidence:

- `pkms-rag::connect` publicly returns `rusqlite::Connection`; public ingest,
  status, search, dense-search, and retrieve functions all accept it.
- Both CLI adapters and HTTP routes repeat “connect, call operation, map
  result” orchestration.
- `IndexProgress.phase` and `current_step` are `String`; the CLI detects failure
  with `progress.phase == "error"`.
- Synchronous indexing has parallel provider-vs-provider-config flows in
  `indexer.rs`, and errors are converted into progress strings rather than
  returned as `Result` for synchronous callers.
- `db.rs` is 1,494 lines and owns connection creation, schema setup, ingest,
  search, embedding refresh, cleanup, and status queries.

Why this is costly:

- SQLite becomes part of the public crate contract, making storage refactors
  cross-cutting.
- Invalid state strings are representable and compiler exhaustiveness is lost.
- CLI and HTTP callers must understand low-level storage sequencing.
- Background-status concerns complicate the simpler synchronous use case.

Recommended fix:

1. Add a `RagIndex` facade that owns the database path and opens/uses internal
   connections. Expose `status`, `ingest`, `search`, and `retrieve` methods.
2. Keep transaction/query functions private under a `storage` module. Split
   that module by connection/schema, ingest, query, and cleanup only after the
   facade exists.
3. Replace phase and step strings with serialized enums such as `IndexPhase`
   and `IndexStep`.
4. Make synchronous indexing return `Result<IndexSummary>` while emitting
   progress through a callback. The background adapter may convert errors into
   a stored `IndexProgress::Failed` state.
5. Consolidate provider construction behind one internal provider-factory path.
6. Validate API limits, token budgets, modes, and finite/non-negative weights
   at the HTTP boundary, then pass trusted typed requests inward.

Keep the existing note-viewer adapter in the umbrella crate. It is a legitimate
dependency inversion that prevents `pkms-rag` and `pkms-web` from depending on
each other. It can be made less repetitive later with a shared HTTP method enum,
but a new generic protocol crate is not currently justified.

### 6. Public API surfaces are broader than the intended contracts

Priority: **medium**

Evidence:

- `crates/pkms/src/lib.rs` publicly exposes every internal module.
- `pkms-org` publicly exposes almost every module plus graph internals.
- `pkms-rag` has a large flat re-export list containing storage helpers,
  embedding implementation details, API state, and application use cases.
- Public helper functions in internal modules are not reported as dead code.
  For example, `extract_date`, `is_overdue`, `is_overdue_on`, `sort_items`, and
  `print_table` in `commands/task_common.rs` have no external call sites in the
  current tree.

Recommended fix:

- Treat workspace crate APIs as real contracts even if crates are not yet
  published independently.
- Expose one small facade per domain use case; make modules private by default.
- For the binary/library pair, expose a narrow application entry point needed
  by `main.rs` and tests instead of every module.
- Add new narrow methods first, migrate consumers, then reduce visibility.
- Use `cargo machete` or an equivalent metadata-based CI check for unused
  dependencies; normal Rust dead-code analysis cannot see unused public API.

Do not blanket-convert all `anyhow::Result` APIs. Typed errors are valuable only
where a caller branches on the error (for example target-not-found, feature
unavailable, index-already-running, or invalid request). Internal command
orchestration can continue using `anyhow` with context.

### 7. Several modules have multiple independent reasons to change

Priority: **medium**

Current hotspots (line counts include inline tests):

| File | Lines | Main issue |
|---|---:|---|
| `pkms-rag/src/db.rs` | 1,494 | storage lifecycle, ingest, queries, cleanup, and tests |
| `pkms-org/src/parser.rs` | 1,219 | parser model, document scan, properties, links, and tests |
| `pkms-web/src/lib.rs` | 980 | public facade plus roughly 780 lines of inline tests |
| `pkms/src/config/mod.rs` | 908 | file schema, precedence, environment, domain mapping, and tests |
| `pkms-db/src/link_check/ssh.rs` | 820 | target parsing, grouping, runtime, auth, host keys, SFTP |
| `pkms/src/commands/task/render.rs` | 801 | several text/table/structured render paths |
| `pkms/src/commands/rag.rs` | 680 | six subcommands, environment resolution, viewer bridge, rendering |
| `pkms/tests/integration/task.rs` | 5,551 | all task integration scenarios in one module |

Recommended splits should follow responsibility, not an arbitrary line limit:

- `pkms-rag/storage/{mod,connection,ingest,query,cleanup}.rs` after introducing
  `RagIndex`.
- `pkms-org/parser/{mod,document,heading,property,link}.rs` while keeping one
  public `parse_note` facade.
- conventional `pkms-web/src/serve/mod.rs` instead of root-level `#[path = ...]`
  wiring; move facade tests to `serve/tests.rs` or focused module tests.
- `pkms/src/cli/rag.rs` and `commands/rag/{mod,render,viewer}.rs`.
- `pkms-db/link_check/ssh/{target,client,auth}.rs`.
- task rendering split into model-to-row projection, text tables, and
  structured output.
- integration task tests split by list/agenda/show-open/mutations/Todoist while
  retaining the single integration test target and shared `TestDb` fixtures.

Avoid splitting a parser or renderer merely to reduce line count. First define
the facade and invariants; otherwise a file split only relocates complexity.

### 8. Boundary enforcement and manifest hygiene lag behind the documentation

Priority: **medium**, low implementation risk

Evidence:

- `scripts/check-crate-boundaries.sh` passes, but it does not reject
  `pkms-rag` from `pkms-org`, `pkms-db`, `pkms-task`, or `pkms-web`, even though
  the documented matrix forbids those edges.
- The all-feature `pkms` check requires `pkms-web` but not `pkms-rag`.
- Direct dependencies that appear unused in current non-test source include:
  `regex`, `walkdir`, `glob`, and `tiktoken-rs` in `pkms`; `dirs` in
  `pkms-org`, `pkms-db`, and `pkms-task`; and the `pkms` dev dependency
  `proptest`.
- `serde_json` in `pkms-task` is used only by Todoist-gated modules and can be
  optional with the `todoist` feature.
- `pkms-web` defines an empty `web` feature enabled by default even though the
  umbrella's optional dependency already gates the crate.
- `CommandContext::load_graph`/`load_workspace` have no call sites, and
  `DbCommandConfig::task_states` is constructed but unused.

Recommended fix:

- Encode the complete allowed/forbidden matrix in the boundary script,
  including RAG.
- Prefer checking direct manifest edges from `cargo metadata`; retain a
  transitive check if the policy intentionally forbids indirect reachability.
- Remove unused dependencies one crate at a time and verify all feature sets.
- Remove the empty `pkms-web/web` feature unless it is given a real semantic
  purpose.
- Delete unused loaders, fields, and public helpers after confirming no
  external compatibility requirement.

### 9. Use types where they remove branches, not everywhere

Priority: **medium**

Good examples already exist: `NoteId`, `LinkTarget`, `TaskId`, `TaskState`,
`TaskPriority`, `TaskDateValue`, and `RetrieveMode` make invalid combinations
harder to express.

The next high-value typed boundaries are:

- `IndexPhase` and `IndexStep` instead of strings;
- `SocketAddr`/`IpAddr` instead of server host strings after CLI parsing;
- a typed editor target (`PathBuf`, optional line, display title);
- a typed field selection enum for `resolve --fields`;
- validated retrieval limits/token budgets/weights; and
- paths kept as `PathBuf` internally and converted to strings only in output
  DTOs.

Low-value work would include newtyping every title/tag string or creating a
trait for every command. Prefer an enum/newtype only when it removes repeated
validation, string comparisons, or ambiguous parameters.

## Incremental implementation plan

Each task below is intended to be an independent, behavior-preserving change.
Refactoring and new behavior should not be mixed in the same commit.

### Phase 1: Protect contracts and enforce the intended graph

#### Task 1: Freeze critical output and task-ID contracts

Description: Add focused contract coverage before moving ownership. Cover
single/multiple JSON shapes, NDJSON fields, task show/open resolution, canonical
ID stability, and unhealthy check exit status.

Acceptance criteria:

- Existing text/JSON/NDJSON output is represented by integration assertions or
  snapshots.
- Task IDs remain identical across list, agenda, show, and mutation fixtures.
- Tests fail if check exit-code mapping or structured output shape changes.

Verification:

```bash
cargo test --test integration task
cargo test --test integration check
cargo test --test integration snapshot
```

Dependencies: none.
Likely files: focused modules under `crates/pkms/tests/integration/` (split into
batches of at most five files).
Scope: medium.

#### Task 2: Complete crate-boundary enforcement

Description: Make the script encode the documented matrix, including all RAG
edges, and document whether direct or transitive reachability is enforced.

Acceptance criteria:

- A fixture or temporary manifest edit proves each forbidden edge is detected.
- `pkms-rag` is present in the all-feature umbrella tree.
- The normal boundary check passes.

Verification: `scripts/check-crate-boundaries.sh`.
Dependencies: none.
Likely files: `scripts/check-crate-boundaries.sh`, `docs/architecture.md`,
`docs/development.md`.
Scope: small.

#### Task 3: Remove obvious unused surface and dependencies

Description: Use separate small commits per crate to remove dead helpers,
unused loaders/config fields, unused manifest entries, and the empty web
feature.

Acceptance criteria:

- `CommandContext` and DB config expose only used capabilities.
- No direct manifest dependency lacks a source/test use without an explicit
  rationale.
- Default and all-feature builds retain the same commands.

Verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Dependencies: Task 2.
Likely files: no more than three crate manifests plus `Cargo.lock` per commit;
then a separate commit for `command_context.rs`, `config/mod.rs`, and
`task_common.rs`.
Scope: several small changes.

### Phase 2: Correct task and process ownership

#### Task 4: Return domain health instead of process exit codes

Description: Remove `ExitCode` from `pkms-db` check models and map health to a
process code in the CLI adapter.

Acceptance criteria:

- `pkms-db` has no `std::process::ExitCode` dependency.
- Healthy/unhealthy text and structured output are unchanged.
- Existing exit-code integration tests pass.

Verification: `cargo test --test integration check`.
Dependencies: Task 1.
Likely files: `pkms-db/commands/check.rs`, `check/model.rs`,
`pkms/commands/check.rs`, focused tests.
Scope: small.

#### Task 5: Isolate editor execution in the umbrella crate

Description: Introduce a typed editor target and move command parsing, process
launch, and printing out of `pkms-db`. Adapt task open and viewer callbacks in
small sequential commits.

Acceptance criteria:

- No domain crate writes process output or starts the editor.
- Task open and web “Open in Emacs” behavior remain unchanged.
- Editor parsing and non-zero child status have focused tests.

Verification:

```bash
cargo test --test integration task_open
cargo test --features web --test integration serve
```

Dependencies: Task 1.
Likely files per commit: new `pkms/src/editor.rs`, task open adapter,
`pkms-web/src/lib.rs`, viewer/open tests; then delete
`pkms-db/src/commands/open.rs` in a follow-up.
Scope: medium.

#### Task 6: Move raw task projection and policy into `pkms-task`

Description: First add task-side projection over `Corpus`/parsed headings, then
switch canonical indexing/providers, then delete the policy-bearing org module.

Acceptance criteria:

- `pkms-org` no longer contains agenda modes, closed-state filtering, a task
  clock, or overdue policy.
- Canonical ID ordering and all filters produce identical results.
- Local task edits still go through typed `pkms-org` mutation requests.

Verification:

```bash
cargo test -p pkms-task
cargo test --test integration task
cargo test --workspace --all-features
```

Dependencies: Task 1.
Likely files per commit: `pkms-task/src/task_index.rs`, a new task projection
module, `pkms-task/src/config.rs`, focused tests; deletion/export cleanup in a
separate commit touching `pkms-org/src/lib.rs` and task extraction files.
Scope: medium, sequential.

#### Task 7: Move task show into the task domain

Description: Move show execution and its task-ID-aware output model from
`pkms-db` to `pkms-task`; move the CLI adapter under `commands/task/`.

Acceptance criteria:

- `pkms-db` contains no task-ID or task parent/child model.
- `pkms-task` resolves canonical IDs and show relationships without umbrella
  translation structs.
- Text and structured show output remain byte/schema compatible.

Verification: focused task-show tests plus `cargo test --test integration task`.
Dependencies: Task 6.
Likely files per commit: task show module, task facade, task adapter, focused
tests; DB deletion/export cleanup separately.
Scope: medium.

### Phase 3: Make org loading a clear, narrow boundary

#### Task 8: Add graph query methods and migrate direct field access

Description: Add read-only query methods without changing storage, then migrate
one consumer group at a time before privatizing fields.

Acceptance criteria:

- External crates do not access graph index fields directly.
- Graph invariants can be changed without editing every consumer.
- No additional filesystem scan is introduced.

Verification: `cargo test -p pkms-org`, followed by each migrated crate's tests.
Dependencies: Tasks 6 and 7.
Likely files: `pkms-org/src/graph/mod.rs` plus at most four consumer modules per
commit.
Scope: several small/medium batches.

#### Task 9: Introduce the org snapshot and focused configs

Description: Make one fresh scan produce a snapshot used for parsed content and
graph queries. Split scan/link/note-directory configuration by consumer need.

Acceptance criteria:

- A read-only command can obtain graph metadata and content from one snapshot.
- Graph loading no longer accepts note-creation directories.
- Fresh-scan behavior and ignore/home path semantics remain unchanged.

Verification:

```bash
cargo test -p pkms-org
cargo test -p pkms-db
cargo test -p pkms-task
```

Dependencies: Task 8.
Likely files per slice: `corpus.rs`, `workspace.rs` (or new `snapshot.rs`),
`graph/mod.rs`, and one consumer/test module.
Scope: medium, multi-slice.

#### Task 10: Isolate token counting

Description: Create the focused token leaf crate, migrate RAG and get, then
remove tokenization from `pkms-org`.

Acceptance criteria:

- Standalone `pkms-web` and `pkms-task` dependency trees do not contain
  `tiktoken-rs`.
- Get token estimates and RAG token budgets remain unchanged.
- The new crate exports only encoding/count/truncate operations.

Verification:

```bash
cargo test -p pkms-tokens
cargo test --test integration get
cargo test --features rag --test integration rag
cargo tree -p pkms-web --no-default-features
```

Dependencies: Task 9 is helpful but not required.
Likely files per commit: new crate manifest/lib and workspace manifest, then
one consumer crate at a time.
Scope: medium.

### Phase 4: Make runtime inputs and RAG internals explicit

#### Task 11: Inject process inputs into configuration/adapters

Description: Add `load_from` and pure precedence functions, then migrate RAG
and SSH environment resolution without changing precedence.

Acceptance criteria:

- Unit tests do not read or mutate the real process environment.
- Domain crates still contain no environment reads.
- CLI/config/environment precedence remains documented and tested.

Verification: config, check, RAG, and all-feature integration tests.
Dependencies: Phase 2.
Likely files per slice: `config/mod.rs`, a new environment module, config tests;
then `commands/check.rs` or `commands/rag.rs` and its tests.
Scope: medium, two or three slices.

#### Task 12: Add `RagIndex` and hide SQLite connections

Description: Introduce the storage facade, migrate CLI and HTTP callers, then
make connection/query helpers private.

Acceptance criteria:

- No public API outside the storage module accepts/returns
  `rusqlite::Connection`.
- CLI and HTTP use the same ingest/search/retrieve service operations.
- Transaction, stale-row cleanup, FTS, and dense retrieval behavior are
  unchanged.

Verification: `cargo test -p pkms-rag` and RAG integration tests.
Dependencies: Task 1.
Likely files per slice: `db.rs`/new storage facade, `lib.rs`, one caller
(`commands/rag.rs` or `api/routes.rs`), and focused tests.
Scope: medium, sequential.

#### Task 13: Type and simplify index progress

Description: Add typed phases/steps and one provider-factory execution path;
separate synchronous `Result` from background stored status.

Acceptance criteria:

- No code compares phase/step string literals.
- Synchronous failures are returned as errors with their context chain.
- Background duplicate-start and failure status behavior remains compatible at
  the HTTP/JSON boundary.

Verification: `cargo test -p pkms-rag indexer` and RAG integration tests.
Dependencies: Task 12.
Likely files: `models.rs`, `indexer.rs`, `commands/rag.rs`, API state/routes,
focused tests.
Scope: medium.

### Phase 5: Split modules and narrow public facades

#### Task 14: Split only confirmed responsibility hotspots

Description: Perform mechanical module moves after the relevant facades are in
place. Start with RAG storage, web tests/module wiring, and task integration
tests; split the parser and SSH client only when active changes require it.

Acceptance criteria:

- Each new module has one stated responsibility.
- Public paths are preserved or deliberately migrated with compiler help.
- No behavior change or unrelated formatting is mixed into module-move commits.

Verification: full pre-commit gate after every area.
Dependencies: the relevant earlier facade task.
Likely files: keep each move to one module family and at most five hand-edited
files; use mechanical moves for larger changes.
Scope: several small/medium changes.

#### Task 15: Narrow facades and update architecture documentation

Description: Reduce visibility after consumers use the new APIs, then record
accepted ownership decisions in architecture docs/ADRs.

Acceptance criteria:

- Each crate root exposes only supported use cases and shared contract types.
- Internal indexes/storage helpers are private.
- `docs/architecture.md`, per-crate docs, and the boundary script describe the
  same matrix and ownership rules.

Verification: full pre-commit gate and generated/public API inspection.
Dependencies: all ownership-changing tasks.
Likely files: one crate facade plus its crate doc per commit; final architecture
doc/ADR separately.
Scope: several small changes.

## Checkpoints

After Phase 1:

- Existing behavior is protected.
- The dependency matrix and manifests are trustworthy.
- No architecture move has happened yet.

After Phase 2:

- A task change should normally stay in `pkms-task` plus its CLI adapter.
- `pkms-db` has no task-only or process-only behavior.

After Phase 3:

- `pkms-org` is a narrow org snapshot/parser/graph/edit crate.
- Graph internals and tokenization no longer leak through it.

After Phase 4:

- Runtime inputs are deterministic and testable.
- RAG callers no longer know about SQLite connections or string state
  transitions.

After Phase 5:

- Directory structure, public facades, enforcement, and docs all express the
  same architecture.

Every checkpoint should run the project's full gate:

```bash
cargo fmt --all -- --check
scripts/check-crate-boundaries.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Canonical task IDs shift during ownership moves | High | Freeze cross-command ID fixtures first; move code without changing sort/filter rules |
| Text/JSON/NDJSON contracts change | High | Snapshot/schema tests before moves; keep output DTO field names and ordering |
| Org mutations damage user files | High | Keep edits in `pkms-org`; retain dry-run and byte-level fixture tests; do not combine with parser rewrites |
| Fresh scan becomes slower or turns into hidden caching | High | Benchmark/inspect scan count per command; explicitly prohibit persistent state |
| Feature combinations regress | Medium-high | Run all-feature gate plus focused default/no-default feature checks after manifest/API changes |
| Public API narrowing breaks unknown consumers | Medium | Add replacement APIs first, deprecate if crates are externally consumed, reduce visibility last |
| File splits create churn without clarity | Medium | Split only after a facade/responsibility is defined; use mechanical moves and separate commits |
| New token crate becomes a generic core crate | Low | Keep its API limited to encoding/count/truncate and forbid domain dependencies |

## Non-goals

- No persistent graph cache, database, daemon, or watch mode.
- No rewrite of the org parser.
- No async conversion of the whole CLI.
- No generic command trait or service-container framework.
- No blanket replacement of `anyhow`.
- No new `pkms-core` catch-all crate.
- No behavior changes mixed into boundary moves.

## Recommended first milestone

The first implementation milestone should stop after Tasks 1-7. It offers the
best ratio of clarity to risk: contracts become explicit, enforcement improves,
dead surface is removed, process behavior returns to the umbrella, and all task
semantics live in `pkms-task`. Review that result before starting the larger
snapshot and RAG encapsulation work.
