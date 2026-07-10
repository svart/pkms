//! Task management types and logic for pkms.

mod clock;
mod common;
mod config;
mod execution;
mod filter;
mod id;
mod model;
mod modifiers;
mod mutation;
mod pkms;
mod projection;
mod provider;
mod providers;
mod scope;
mod show;
mod task_index;
#[cfg(feature = "todoist")]
mod todoist;
#[cfg(feature = "todoist")]
mod todoist_mutation;
mod todoist_provider;

pub use clock::TaskClock;
pub use common::{
    AgendaWindow, TASK_SORT_FIELD_HELP, TaskGroupField, TaskSortField, agenda_day_section_label,
    agenda_window_cutoff, apply_limit, date_in_agenda_window, group_task_items,
    parse_task_group_field, parse_task_sort_fields, retain_agenda_window_task_items_on,
    retain_upcoming_task_items_on, sort_task_items,
};
pub use config::{PkmsTaskConfig, TaskStateConfig, TodoistProviderConfig};
pub use execution::{
    AgendaExecution, AgendaRequest, TaskListExecution, TaskListItems, TaskListRequest,
    collect_shortcut_items_on, execute_task_agenda, execute_task_list,
};
pub use filter::{SourceSelection, TaskFilters, parse_task_filters, parse_task_filters_on};
pub use id::TaskId;
pub use model::{
    TaskDate, TaskDateValue, TaskItem, TaskPriority, TaskProperty, TaskSourceKind, TaskState,
    TaskStatus,
};
pub use modifiers::TaskModifierSpec;
pub use mutation::{
    TaskModOutput, TaskStateChangeOutput, add_pkms_task, mod_pkms_task, postpone_pkms_task,
    set_pkms_state, unsupported_task_source,
};
pub use provider::{TaskListView, TaskMetadataRow};
pub use providers::{MetadataKind, TaskProviderEnvironment, collect_task_metadata_on};
pub use show::{
    HeadingTarget, ShowOptions, ShowOutput, TaskIdEntry, execute as execute_show,
    render_text as render_show_text,
};
pub use task_index::{
    CanonicalTaskEntry, TaskLocation, all_task_entries, resolve_canonical_task_id,
};
pub use todoist_provider::get_item as get_todoist_item;

#[cfg(feature = "todoist")]
pub use todoist_mutation::{
    TodoistDoneOutput, TodoistStateOutput, add_todoist_task, close_todoist_task, mod_todoist_task,
    postpone_todoist_task, set_todoist_state,
};
