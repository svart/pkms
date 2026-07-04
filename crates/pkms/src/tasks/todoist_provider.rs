use crate::config::ResolvedConfig;
use crate::tasks::filter::TaskFilters;
use crate::tasks::model::TaskItem;
use crate::tasks::provider::TaskMetadataRow;
use anyhow::Result;
#[cfg(feature = "todoist")]
use pkms_task::config::TodoistProviderConfig;

pub use pkms_task::todoist_provider::task_view_filter;

#[cfg(feature = "todoist")]
fn todoist_provider_config(config: &ResolvedConfig) -> Result<TodoistProviderConfig> {
    Ok(TodoistProviderConfig {
        org: config.org_config(),
        token: config.todoist_token()?,
        api_base_url: config.todoist_api_base_url(),
        default_filter: config.todoist_default_filter().map(str::to_string),
    })
}

#[cfg(feature = "todoist")]
pub fn list_items(config: &ResolvedConfig, filters: &TaskFilters) -> Result<Vec<TaskItem>> {
    pkms_task::todoist_provider::list_items(&todoist_provider_config(config)?, filters)
}

#[cfg(not(feature = "todoist"))]
pub fn list_items(_config: &ResolvedConfig, _filters: &TaskFilters) -> Result<Vec<TaskItem>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

#[cfg(feature = "todoist")]
pub fn project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    pkms_task::todoist_provider::project_rows(&todoist_provider_config(config)?)
}

#[cfg(not(feature = "todoist"))]
pub fn project_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

#[cfg(feature = "todoist")]
pub fn label_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    pkms_task::todoist_provider::label_rows(&todoist_provider_config(config)?)
}

#[cfg(not(feature = "todoist"))]
pub fn label_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}
