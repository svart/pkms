use crate::config::ResolvedConfig;
use anyhow::Result;
use pkms_task::{PkmsTaskConfig, TaskClock, TaskMetadataRow, TaskProviderEnvironment};

pub(super) use pkms_task::MetadataKind;

impl TaskProviderEnvironment for ResolvedConfig {
    fn pkms_config(&self) -> PkmsTaskConfig {
        self.pkms_task_config()
    }
}

pub(super) fn collect_task_metadata(
    config: &ResolvedConfig,
    raw_filters: &[String],
    kind: MetadataKind,
    clock: TaskClock,
) -> Result<Vec<TaskMetadataRow>> {
    pkms_task::collect_task_metadata_on(config, raw_filters, kind, clock)
}
