use crate::config::ResolvedConfig;
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{SourceSelection, TaskFilters};
use crate::tasks::model::{TaskItem, TaskSourceKind};
use crate::tasks::pkms;
use crate::tasks::provider::{
    TaskListView, TaskMetadataRow, TaskProvider, TaskProviderContext, TaskQuery,
};
use crate::tasks::todoist_provider;
use anyhow::Result;
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
            TaskListView::All => pkms::list_items_on(self.context.config, query.clock),
            TaskListView::Agenda => pkms::agenda_items_for_clock(
                self.context.config,
                false,
                false,
                false,
                false,
                query.clock,
            ),
            TaskListView::Today => pkms::agenda_items_for_clock(
                self.context.config,
                true,
                false,
                false,
                false,
                query.clock,
            ),
            TaskListView::Week => pkms::agenda_items_for_clock(
                self.context.config,
                false,
                true,
                false,
                false,
                query.clock,
            ),
            TaskListView::Overdue => pkms::agenda_items_for_clock(
                self.context.config,
                false,
                false,
                true,
                false,
                query.clock,
            ),
            TaskListView::Upcoming { days } => {
                let mut items = pkms::agenda_items_for_clock(
                    self.context.config,
                    false,
                    false,
                    false,
                    true,
                    query.clock,
                )?;
                super::retain_upcoming_task_items_on(&mut items, days, query.clock.today);
                Ok(items)
            }
            TaskListView::Inbox => pkms::collect_inbox_items_on(self.context.config, query.clock),
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
                    .or_else(|| todoist_provider::task_view_filter(TaskListView::Agenda)),
            ),
            TaskListView::Today
            | TaskListView::Week
            | TaskListView::Overdue
            | TaskListView::Upcoming { .. }
            | TaskListView::Inbox => {
                let shortcut = todoist_provider::task_view_filter(query.view);
                query
                    .filters
                    .with_todoist_filter(query.filters.todoist_filter.clone().or(shortcut))
            }
        };
        todoist_provider::list_items(self.context.config, &filters)
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_provider::project_rows(self.context.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_provider::label_rows(self.context.config)
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
    clock: TaskClock,
) -> Result<Vec<TaskItem>> {
    let query = TaskQuery {
        filters: filters.clone(),
        view,
        clock,
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
