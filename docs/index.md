# Documentation Index

Use this page as the entry point after the root `README.md` or `AGENTS.md`.
The root files stay compact; detailed user, agent, and crate documentation
lives here under `docs/`.

## User Guides

- [Installation](installation.md): install from source and build optional
  feature sets.
- [Configuration](configuration.md): database root resolution, note
  directories, task settings, Todoist, RAG, and diagnostics.
- [Notes Database Format](database-format.md): org-roam note shape, IDs, links,
  filetags, heading nodes, and task headings.
- [Command Reference](commands.md): grouped CLI commands and flags. Run
  `pkms <command> --help` for the authoritative parser view.
- [Note Database Commands](note-database-commands.md): practical scenarios for
  health checks, lookup, search, graph navigation, note creation, extraction,
  repair, and pipelines.
- [TODO and Agenda](todo-agenda.md): task list, agenda, filters, columns, task
  IDs, mutations, and Todoist-backed task usage.
- [Web Viewer](web.md): how to build and run `pkms serve`.
- [RAG Retrieval](rag.md): how to build, index, retrieve, and serve local
  retrieval over notes.
- [Maintenance Workflows](workflows.md): repeatable note maintenance and task
  workflows.
- [Pipelining](pipelining.md): NDJSON producer and consumer recipes.
- [JSON and NDJSON Output](json-output.md): structured output contracts.

## Architecture and Agent Guides

- [Architecture](architecture.md): workspace boundaries, command flow, data
  flow, and feature flags.
- [Crate Boundary Decision](adr/0001-maintainable-crate-boundaries.md): accepted
  ownership, loading, runtime-input, storage, and facade decisions.
- [Development](development.md): local development loop, checks, project
  structure, command conventions, and release workflow.
- [Task System Design](task-system.md): implementation rules for task IDs,
  source semantics, filters, and mutations.
- [Refactoring Assessment and Plan](refactoring-assessment-and-plan.md):
  evidence-backed crate-boundary and maintainability review with a staged
  implementation plan.

## Crate Documentation

- [pkms](crates/pkms.md): umbrella binary crate for CLI parsing, config,
  dispatch, output, and cross-domain command orchestration.
- [pkms-org](crates/pkms-org.md): org discovery, parsing, graph, snapshots, and
  edit primitives.
- [pkms-db](crates/pkms-db.md): note database command logic and link checks.
- [pkms-task](crates/pkms-task.md): task domain model, IDs, filters, providers,
  mutations, and Todoist support.
- [pkms-rag](crates/pkms-rag.md): local retrieval models, SQLite index,
  embeddings, search, retrieval, and RAG HTTP API.
- [pkms-web](crates/pkms-web.md): local note viewer, HTML rendering, assets, and
  foreground HTTP serving.
- [pkms-tokens](crates/pkms-tokens.md): token encoding, counting, and
  truncation leaf utilities.

## Where To Start

| Work | Start here | Then open |
|------|------------|-----------|
| Use `pkms` on notes | [Command Reference](commands.md) | [Note Database Commands](note-database-commands.md) |
| Work with tasks | [TODO and Agenda](todo-agenda.md) | [Task System Design](task-system.md) for implementation |
| Run local retrieval | [RAG Retrieval](rag.md) | [pkms-rag crate](crates/pkms-rag.md) |
| Run the web viewer | [Web Viewer](web.md) | [pkms-web crate](crates/pkms-web.md) |
| Change command behavior | [Development](development.md) | [pkms crate](crates/pkms.md) and the domain crate |
| Change parsing or graph logic | [pkms-org crate](crates/pkms-org.md) | [Notes Database Format](database-format.md) |
| Change database checks | [pkms-db crate](crates/pkms-db.md) | [Note Database Commands](note-database-commands.md) |
| Change tasks | [pkms-task crate](crates/pkms-task.md) | [Task System Design](task-system.md) |
