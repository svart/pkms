use crate::config::ResolvedConfig;
use crate::tasks::filter::TaskFilters;
use crate::tasks::model::TaskItem;
#[cfg(feature = "todoist")]
use crate::tasks::model::TaskSourceKind;
use crate::tasks::provider::{TaskListView, TaskMetadataRow};
use anyhow::Result;

pub fn task_view_filter(view: TaskListView) -> Option<String> {
    match view {
        TaskListView::All => None,
        TaskListView::Agenda => Some("!no date".to_string()),
        TaskListView::Today => Some("today".to_string()),
        TaskListView::Week => Some("next 7 days".to_string()),
        TaskListView::Overdue => Some("overdue".to_string()),
        TaskListView::Upcoming { days } => Some(format!("due after: today & next {days} days")),
        TaskListView::Inbox => Some("#Inbox".to_string()),
    }
}

#[cfg(feature = "todoist")]
pub fn list_items(config: &ResolvedConfig, filters: &TaskFilters) -> Result<Vec<TaskItem>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    let tasks = match filters
        .todoist_filter
        .as_deref()
        .or_else(|| config.todoist_default_filter())
    {
        Some(filter) => client.filter_tasks(filter)?,
        None => client.list_tasks()?,
    };
    let metadata = if tasks.iter().any(|task| task.project_id.is_some()) {
        Some(crate::tasks::todoist::TodoistMetadata::new(
            client.list_projects()?,
        ))
    } else {
        None
    };
    let mut items = tasks
        .into_iter()
        .map(|task| crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref()))
        .collect::<Vec<_>>();
    crate::tasks::todoist::enrich_items_with_pkms_notes(config, &mut items)?;
    Ok(items)
}

#[cfg(not(feature = "todoist"))]
pub fn list_items(_config: &ResolvedConfig, _filters: &TaskFilters) -> Result<Vec<TaskItem>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

#[cfg(feature = "todoist")]
pub fn project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    Ok(client
        .list_projects()?
        .into_iter()
        .map(|project| TaskMetadataRow {
            source: TaskSourceKind::Todoist,
            id: project.id,
            name: project.name,
            count: None,
        })
        .collect())
}

#[cfg(not(feature = "todoist"))]
pub fn project_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

#[cfg(feature = "todoist")]
pub fn label_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let token = crate::tasks::todoist::ensure_enabled(config)?;
    let client =
        crate::tasks::todoist::TodoistClient::with_base_url(config.todoist_api_base_url(), token);
    Ok(client
        .list_labels()?
        .into_iter()
        .map(|label| TaskMetadataRow {
            source: TaskSourceKind::Todoist,
            id: label.id,
            name: label.name,
            count: None,
        })
        .collect())
}

#[cfg(not(feature = "todoist"))]
pub fn label_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}
