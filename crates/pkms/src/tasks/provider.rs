use crate::config::ResolvedConfig;

pub use pkms_task::provider::{TaskListView, TaskMetadataRow, TaskProvider, TaskQuery};

pub struct TaskProviderContext<'a> {
    pub config: &'a ResolvedConfig,
}
