use crate::config::ResolvedConfig;
use crate::tasks::filter::{SourceSelection, TaskFilters};
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms;
use crate::tasks::provider::{
    TaskListView, TaskMetadataRow, TaskProvider, TaskProviderContext, TaskQuery,
};
use anyhow::Result;
use chrono::NaiveDate;
use std::collections::BTreeMap;

struct PkmsTaskProvider<'a> {
    context: TaskProviderContext<'a>,
}

struct TodoistTaskProvider<'a> {
    context: TaskProviderContext<'a>,
}

impl<'a> PkmsTaskProvider<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            context: TaskProviderContext { config },
        }
    }
}

impl<'a> TodoistTaskProvider<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            context: TaskProviderContext { config },
        }
    }
}

struct TaskProviders<'a> {
    pkms: PkmsTaskProvider<'a>,
    todoist: TodoistTaskProvider<'a>,
}

impl<'a> TaskProviders<'a> {
    fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            pkms: PkmsTaskProvider::new(config),
            todoist: TodoistTaskProvider::new(config),
        }
    }

    fn list(&self, source: SourceSelection, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        tracing::debug!(
            source = ?source,
            view = ?query.view,
            has_todoist_filter = query.filters.todoist_filter.is_some(),
            "collecting task items from providers"
        );
        match source {
            SourceSelection::Pkms => {
                let items = self.pkms.list(query)?;
                tracing::debug!(
                    source = "pkms",
                    item_count = items.len(),
                    "provider returned tasks"
                );
                Ok(items)
            }
            SourceSelection::Todoist => {
                let items = self.todoist.list(query)?;
                tracing::debug!(
                    source = "todoist",
                    item_count = items.len(),
                    "provider returned tasks"
                );
                Ok(items)
            }
            SourceSelection::All => {
                let mut items = self.pkms.list(query)?;
                let pkms_count = items.len();
                let todoist_items = self.todoist.list(query)?;
                let todoist_count = todoist_items.len();
                items.extend(todoist_items);
                tracing::debug!(
                    pkms_count,
                    todoist_count,
                    total_count = items.len(),
                    "providers returned combined tasks"
                );
                Ok(items)
            }
        }
    }

    fn metadata(
        &self,
        source: SourceSelection,
        kind: MetadataKind,
    ) -> Result<Vec<TaskMetadataRow>> {
        let collect = |provider: &dyn TaskProvider| match kind {
            MetadataKind::Projects => provider.projects(),
            MetadataKind::Tags => provider.tags(),
        };

        match source {
            SourceSelection::Pkms => collect(&self.pkms),
            SourceSelection::Todoist => collect(&self.todoist),
            SourceSelection::All => {
                let mut rows = collect(&self.pkms)?;
                rows.extend(collect(&self.todoist)?);
                Ok(rows)
            }
        }
    }
}

impl TaskProvider for PkmsTaskProvider<'_> {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Pkms
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        match query.view {
            TaskListView::All => pkms::list_items(self.context.config),
            TaskListView::Agenda => pkms::agenda_items_for_on(
                self.context.config,
                false,
                false,
                false,
                false,
                query.today,
            ),
            TaskListView::Today => pkms::agenda_items_for_on(
                self.context.config,
                true,
                false,
                false,
                false,
                query.today,
            ),
            TaskListView::Week => pkms::agenda_items_for_on(
                self.context.config,
                false,
                true,
                false,
                false,
                query.today,
            ),
            TaskListView::Overdue => pkms::agenda_items_for_on(
                self.context.config,
                false,
                false,
                true,
                false,
                query.today,
            ),
            TaskListView::Upcoming { days } => {
                let mut items = pkms::agenda_items_for_on(
                    self.context.config,
                    false,
                    false,
                    false,
                    true,
                    query.today,
                )?;
                super::retain_upcoming_task_items_on(&mut items, days, query.today);
                Ok(items)
            }
            TaskListView::Inbox => pkms::collect_inbox_items(self.context.config),
        }
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_project_rows(self.context.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_tag_rows(self.context.config)
    }
}

impl TaskProvider for TodoistTaskProvider<'_> {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Todoist
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        let filters = match query.view {
            TaskListView::All => query.filters.clone(),
            TaskListView::Agenda => query.filters.with_todoist_filter(
                query
                    .filters
                    .todoist_filter
                    .clone()
                    .or_else(|| task_view_todoist_filter(TaskListView::Agenda)),
            ),
            TaskListView::Today
            | TaskListView::Week
            | TaskListView::Overdue
            | TaskListView::Upcoming { .. }
            | TaskListView::Inbox => {
                let shortcut = task_view_todoist_filter(query.view);
                query
                    .filters
                    .with_todoist_filter(query.filters.todoist_filter.clone().or(shortcut))
            }
        };
        collect_todoist_items(self.context.config, &filters)
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_project_rows(self.context.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_label_rows(self.context.config)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MetadataKind {
    Projects,
    Tags,
}

pub(super) fn collect_task_items(
    config: &ResolvedConfig,
    filters: &TaskFilters,
    view: TaskListView,
    today: NaiveDate,
) -> Result<Vec<TaskItem>> {
    let query = TaskQuery {
        filters: filters.clone(),
        view,
        today,
    };
    let items = TaskProviders::new(config).list(filters.source, &query)?;
    tracing::debug!(
        source = ?filters.source,
        view = ?view,
        item_count = items.len(),
        "collected task items"
    );
    Ok(items)
}

pub(super) fn collect_task_metadata(
    config: &ResolvedConfig,
    source: SourceSelection,
    kind: MetadataKind,
) -> Result<Vec<TaskMetadataRow>> {
    TaskProviders::new(config).metadata(source, kind)
}

fn task_view_todoist_filter(view: TaskListView) -> Option<String> {
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
fn collect_todoist_items(config: &ResolvedConfig, filters: &TaskFilters) -> Result<Vec<TaskItem>> {
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
    Ok(tasks
        .into_iter()
        .map(|task| crate::tasks::todoist::task_to_item_with_metadata(task, metadata.as_ref()))
        .collect::<Vec<_>>())
    .and_then(|mut items| {
        super::enrich_todoist_items_with_pkms_notes(config, &mut items)?;
        Ok(items)
    })
}

#[cfg(not(feature = "todoist"))]
fn collect_todoist_items(
    _config: &ResolvedConfig,
    _filters: &TaskFilters,
) -> Result<Vec<TaskItem>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

fn pkms_project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let mut counts = BTreeMap::new();
    for item in pkms::list_items(config)? {
        if let Some(project) = item.project.filter(|project| !project.trim().is_empty()) {
            *counts.entry(project).or_insert(0) += 1;
        }
    }
    Ok(counts
        .into_iter()
        .map(|(project, count)| TaskMetadataRow {
            source: TaskSourceKind::Pkms,
            id: project.clone(),
            name: project,
            count: Some(count),
        })
        .collect())
}

fn pkms_tag_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    let mut counts = BTreeMap::new();
    for item in pkms::list_items(config)? {
        for tag in item.tags {
            if !tag.trim().is_empty() {
                *counts.entry(tag).or_insert(0) += 1;
            }
        }
    }
    Ok(counts
        .into_iter()
        .map(|(tag, count)| TaskMetadataRow {
            source: TaskSourceKind::Pkms,
            id: tag.clone(),
            name: tag,
            count: Some(count),
        })
        .collect())
}

#[cfg(feature = "todoist")]
fn todoist_project_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
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
fn todoist_project_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}

#[cfg(feature = "todoist")]
fn todoist_label_rows(config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
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
fn todoist_label_rows(_config: &ResolvedConfig) -> Result<Vec<TaskMetadataRow>> {
    anyhow::bail!(
        "Todoist support is not available in this build. Rebuild with --features todoist."
    )
}
