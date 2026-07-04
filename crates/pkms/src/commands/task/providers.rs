use crate::config::ResolvedConfig;
use anyhow::Result;
use pkms_task::config::{PkmsTaskConfig, TodoistProviderConfig};
use pkms_task::filter::SourceSelection;
use pkms_task::provider::TaskMetadataRow;
use pkms_task::providers::TaskProviderEnvironment;

pub(super) use pkms_task::providers::MetadataKind;

impl TaskProviderEnvironment for ResolvedConfig {
    fn pkms_config(&self) -> PkmsTaskConfig {
        self.pkms_task_config()
    }

    #[cfg(feature = "todoist")]
    fn todoist_config(&self) -> Result<TodoistProviderConfig> {
        Ok(TodoistProviderConfig {
            org: self.org_config(),
            token: self.todoist_token()?,
            api_base_url: self.todoist_api_base_url(),
            default_filter: self.todoist_default_filter().map(str::to_string),
        })
    }

    #[cfg(not(feature = "todoist"))]
    fn todoist_config(&self) -> Result<TodoistProviderConfig> {
        Ok(TodoistProviderConfig {
            org: self.org_config(),
            token: String::new(),
            api_base_url: String::new(),
            default_filter: None,
        })
    }
}

pub(super) fn collect_task_metadata(
    config: &ResolvedConfig,
    source: SourceSelection,
    kind: MetadataKind,
) -> Result<Vec<TaskMetadataRow>> {
    pkms_task::providers::collect_task_metadata(config, source, kind)
}
