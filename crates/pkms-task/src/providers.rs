use crate::clock::TaskClock;
use crate::common::retain_upcoming_task_items_on;
use crate::config::PkmsTaskConfig;
use crate::filter::TaskFilters;
use crate::model::{TaskItem, TaskSourceKind};
use crate::pkms;
use crate::provider::{TaskListView, TaskMetadataRow, TaskProvider, TaskQuery};
use anyhow::Result;
use std::collections::BTreeMap;

pub trait TaskProviderEnvironment {
    fn pkms_config(&self) -> PkmsTaskConfig;
}

struct PkmsTaskProvider {
    config: PkmsTaskConfig,
}

impl PkmsTaskProvider {
    fn new(config: PkmsTaskConfig) -> Self {
        Self { config }
    }
}

impl TaskProvider for PkmsTaskProvider {
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

#[derive(Debug, Clone, Copy)]
pub enum MetadataKind {
    Projects,
    Tags,
}

pub fn collect_task_items<E: TaskProviderEnvironment>(
    environment: &E,
    _filters: &TaskFilters,
    view: TaskListView,
    clock: TaskClock,
) -> Result<Vec<TaskItem>> {
    let query = TaskQuery { view, clock };
    let items = PkmsTaskProvider::new(environment.pkms_config()).list(&query)?;
    tracing::debug!(view = ?view, item_count = items.len(), "collected local task items");
    Ok(items)
}

pub fn collect_task_metadata_on<E: TaskProviderEnvironment>(
    environment: &E,
    raw_filters: &[String],
    kind: MetadataKind,
    clock: TaskClock,
) -> Result<Vec<TaskMetadataRow>> {
    let filters = crate::filter::parse_task_filters_on(raw_filters, clock.today)?;
    if filters.has_criteria() {
        anyhow::bail!("Task metadata commands only accept source filters.");
    }
    let provider = PkmsTaskProvider::new(environment.pkms_config());
    let mut rows = match kind {
        MetadataKind::Projects => provider.projects()?,
        MetadataKind::Tags => provider.tags()?,
    };
    rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(rows)
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
