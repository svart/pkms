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

pub use clock::TaskClock;
pub use common::{AgendaWindow, TaskGroupField, agenda_day_section_label, apply_limit};
pub use config::{PkmsTaskConfig, TaskStateConfig};
pub use execution::{
    AgendaExecution, AgendaRequest, TaskListExecution, TaskListItems, TaskListRequest,
    collect_shortcut_items_on, execute_task_agenda, execute_task_list,
};
pub use filter::{SourceSelection, TaskFilters, parse_task_filters_on};
pub use id::TaskId;
pub use model::{
    TaskDate, TaskDateValue, TaskItem, TaskPriority, TaskProperty, TaskSourceKind, TaskState,
    TaskStatus,
};
pub use modifiers::TaskModifierSpec;
pub use mutation::{
    TaskModOutput, TaskStateChangeOutput, add_pkms_task, mod_pkms_task, postpone_pkms_task,
    set_pkms_state,
};
pub use provider::{TaskListView, TaskMetadataRow};
pub use providers::{MetadataKind, TaskProviderEnvironment, collect_task_metadata_on};
pub use show::{
    HeadingTarget, ShowOptions, ShowOutput, TaskIdEntry, execute as execute_show,
    render_text as render_show_text,
};
pub use task_index::{CanonicalTaskEntry, TaskLocation, all_task_entries};
