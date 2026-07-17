# Crate: pkms-task

`crates/pkms-task` owns task-domain behavior for local PKMS tasks and optional
Todoist-backed tasks. It does not own CLI parsing or raw org text edits.

## Responsibilities

- Build the canonical local task index.
- Own task state configuration and project parsed org headings into task
  records.
- Assign deterministic PKMS task IDs shared by `task list`, `task agenda`, and
  ID-first actions.
- Parse and apply task filters for source, state, tags, type, priority, dates,
  scope, project, and Todoist raw filters.
- Model source-neutral task items for local and provider-backed tasks.
- Collect tasks from configured providers.
- Produce task-show details and parent/child relationships.
- Plan and execute task mutations such as add, state, done, schedule,
  deadline, priority, tags, project, description, dependency, and postpone.
- Integrate with Todoist when built with the `todoist` feature.
- Send typed local org edit requests into `pkms-org`.

## Main Modules

| Module | Purpose |
|--------|---------|
| `model.rs` | Source-neutral task item and output models. |
| `task_index.rs` | Canonical local task ID assignment and lookup. |
| `id.rs` | Task ID parsing and display rules. |
| `filter.rs` | Task filter parsing and matching. |
| `scope.rs` | PKMS note scope resolution for task views. |
| `modifiers.rs` | Add/modifier parsing for task creation and mutation. |
| `mutation.rs` | Source-neutral mutation planning and result types. |
| `projection.rs` | Converts parsed org headings into task-domain records. |
| `execution.rs` | List and agenda use-case orchestration. |
| `show.rs` | ID-first task-show lookup and typed output. |
| `provider.rs` | Provider trait and source abstraction. |
| `providers.rs` | Provider collection and source selection. |
| `pkms.rs` | Local org-backed task provider behavior. |
| `todoist_provider.rs` | Todoist read provider adapter. |
| `todoist_mutation.rs` | Todoist mutation planning and execution helpers. |
| `todoist.rs` | Todoist API client, behind the `todoist` feature. |
| `clock.rs` | Date and clock abstraction for deterministic behavior. |
| `config.rs` | Task-domain config types. |

## Invariants

- Canonical PKMS task IDs must not depend on filters, priority, dates, tags,
  projects, or relative clock concepts.
- Filtered views may show non-contiguous IDs because excluded tasks still occupy
  their global positions.
- Local writes must go through typed org edit requests instead of ad hoc
  rewriting in command adapters.
- Todoist reads and writes must not log tokens, task content, or descriptions.
- Source-neutral output shape should stay compatible across PKMS and Todoist
  tasks.
- Parsed filter criteria stay private; command adapters receive read-only
  accessors and pass the typed filter value back into task use cases.

## Org-Backed Project Properties

For local PKMS tasks, `PROJECT` is note or heading metadata rather than a
separate project object. Task projection uses the heading-level `PROJECT` value
when present and otherwise falls back to the note-level value.

`pkms-task` owns the policy for explicit `project:` modifiers:

- `task add project:<value>` resolves the destination note from the same fresh
  graph used for the add operation and reads its parsed note-level project.
- If the requested value matches the note project case-insensitively, the new
  heading omits `PROJECT` and inherits the note value.
- If the values differ, or the note has no project, `pkms-task` includes the
  requested value in `OrgTaskInsertSpec` as a heading override.
- `task <ID> mod project:<value>` applies the same comparison. Matching the note
  project produces a typed clear operation for the heading property; a
  different value produces a typed set operation.
- `task <ID> mod project:` always clears the heading-level override. The task
  then inherits the note project when one exists.

The comparison and inheritance decision must remain here. `pkms-org` receives
only the resulting typed set, clear, or omitted property operation and does not
decide task project semantics.

## Boundaries

`pkms-task` may depend on `pkms-org`. It must not depend on `pkms`,
`pkms-db`, `pkms-rag`, or `pkms-web`. CLI argument parsing, table rendering,
and cross-domain open/show dispatch live in `pkms`.

## Related Docs

- [TODO and Agenda](../todo-agenda.md)
- [Task System Design](../task-system.md)
- [Command Reference](../commands.md#tasks)
- [Configuration](../configuration.md)
