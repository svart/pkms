use crate::clock::TaskClock;
use crate::common::retain_upcoming_task_items_on;
use crate::config::{PkmsTaskConfig, TodoistProviderConfig};
use crate::filter::{SourceSelection, TaskFilters};
use crate::model::{TaskItem, TaskSourceKind};
use crate::pkms;
use crate::provider::{TaskListView, TaskMetadataRow, TaskProvider, TaskQuery};
use crate::todoist_provider;
use anyhow::Result;
use std::collections::BTreeMap;

pub trait TaskProviderEnvironment {
    fn pkms_config(&self) -> PkmsTaskConfig;
    fn todoist_config(&self) -> Result<TodoistProviderConfig>;
}

struct PkmsTaskProvider {
    config: PkmsTaskConfig,
}

struct TodoistTaskProvider<'a, E> {
    environment: &'a E,
}

impl PkmsTaskProvider {
    fn new(config: PkmsTaskConfig) -> Self {
        Self { config }
    }
}

impl<'a, E> TodoistTaskProvider<'a, E> {
    fn new(environment: &'a E) -> Self {
        Self { environment }
    }
}

struct TaskProviders<'a, E> {
    pkms: PkmsTaskProvider,
    todoist: TodoistTaskProvider<'a, E>,
}

impl<'a, E: TaskProviderEnvironment> TaskProviders<'a, E> {
    fn new(environment: &'a E) -> Self {
        Self {
            pkms: PkmsTaskProvider::new(environment.pkms_config()),
            todoist: TodoistTaskProvider::new(environment),
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

impl TaskProvider for PkmsTaskProvider {
    fn source(&self) -> TaskSourceKind {
        TaskSourceKind::Pkms
    }

    fn list(&self, query: &TaskQuery) -> Result<Vec<TaskItem>> {
        match query.view {
            TaskListView::All => pkms::list_items_on(&self.config, query.clock),
            TaskListView::Agenda => {
                pkms::agenda_items_for_clock(&self.config, pkms::AgendaView::All, query.clock)
            }
            TaskListView::Today => {
                pkms::agenda_items_for_clock(&self.config, pkms::AgendaView::Today, query.clock)
            }
            TaskListView::Week => {
                pkms::agenda_items_for_clock(&self.config, pkms::AgendaView::Week, query.clock)
            }
            TaskListView::Overdue => {
                pkms::agenda_items_for_clock(&self.config, pkms::AgendaView::Overdue, query.clock)
            }
            TaskListView::Upcoming { days } => {
                let mut items = pkms::agenda_items_for_clock(
                    &self.config,
                    pkms::AgendaView::Upcoming,
                    query.clock,
                )?;
                retain_upcoming_task_items_on(&mut items, days, query.clock.today);
                Ok(items)
            }
            TaskListView::Inbox => pkms::collect_inbox_items_on(&self.config, query.clock),
        }
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_project_rows(&self.config)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        pkms_tag_rows(&self.config)
    }
}

impl<E: TaskProviderEnvironment> TaskProvider for TodoistTaskProvider<'_, E> {
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
        todoist_provider::list_items(&self.environment.todoist_config()?, &filters)
    }

    fn projects(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_provider::project_rows(&self.environment.todoist_config()?)
    }

    fn tags(&self) -> Result<Vec<TaskMetadataRow>> {
        todoist_provider::label_rows(&self.environment.todoist_config()?)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MetadataKind {
    Projects,
    Tags,
}

pub fn collect_task_items<E: TaskProviderEnvironment>(
    environment: &E,
    filters: &TaskFilters,
    view: TaskListView,
    clock: TaskClock,
) -> Result<Vec<TaskItem>> {
    let query = TaskQuery {
        filters: filters.clone(),
        view,
        clock,
    };
    let items = TaskProviders::new(environment).list(filters.source, &query)?;
    tracing::debug!(
        source = ?filters.source,
        view = ?view,
        item_count = items.len(),
        "collected task items"
    );
    Ok(items)
}

pub fn collect_task_metadata<E: TaskProviderEnvironment>(
    environment: &E,
    source: SourceSelection,
    kind: MetadataKind,
) -> Result<Vec<TaskMetadataRow>> {
    TaskProviders::new(environment).metadata(source, kind)
}

fn pkms_project_rows(config: &PkmsTaskConfig) -> Result<Vec<TaskMetadataRow>> {
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

fn pkms_tag_rows(config: &PkmsTaskConfig) -> Result<Vec<TaskMetadataRow>> {
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
