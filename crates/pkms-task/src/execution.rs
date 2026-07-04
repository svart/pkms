use anyhow::Result;
use chrono::NaiveDate;
use pkms_org::Workspace;
use std::collections::BTreeMap;

use crate::clock::TaskClock;
use crate::common::{
    AgendaWindow, TaskGroupField, TaskSortField, group_task_items, parse_task_sort_fields,
    retain_agenda_window_task_items_on, sort_task_items,
};
use crate::config::PkmsTaskConfig;
use crate::filter::{SourceSelection, TaskFilterContext, TaskFilterCriteria, TaskFilters};
use crate::model::TaskItem;
use crate::provider::TaskListView;
use crate::providers::{self, TaskProviderEnvironment};
use crate::scope::ResolvedScope;

pub struct TaskListRequest {
    pub filters: TaskFilters,
    pub scope: Vec<String>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub group: Option<String>,
    pub clock: TaskClock,
}

pub struct AgendaRequest {
    pub filters: TaskFilters,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub window: AgendaWindow,
    pub clock: TaskClock,
    pub view: TaskListView,
}

pub struct TaskListExecution {
    pub source: SourceSelection,
    pub items: TaskListItems,
}

pub enum TaskListItems {
    Flat {
        items: Vec<TaskItem>,
        limit: Option<usize>,
    },
    Grouped {
        group_field: TaskGroupField,
        groups: BTreeMap<String, Vec<TaskItem>>,
        total: usize,
    },
}

pub struct AgendaExecution {
    pub source: SourceSelection,
    pub items: Vec<TaskItem>,
    pub limit: Option<usize>,
    pub window: AgendaWindow,
    pub today: NaiveDate,
}

pub fn execute_task_list<E: TaskProviderEnvironment>(
    environment: &E,
    config: &PkmsTaskConfig,
    request: &TaskListRequest,
) -> Result<TaskListExecution> {
    let mut items = providers::collect_task_items(
        environment,
        &request.filters,
        TaskListView::All,
        request.clock,
    )?;
    let mut criteria = request.filters.criteria.clone();
    criteria.scope = request.scope.clone();
    apply_task_filter_criteria_on(config, &mut items, &criteria, request.clock.today)?;
    let sort = request.sort.as_deref().unwrap_or("date,priority");
    let items = if let Some(group_field) = &request.group {
        let (group_field, groups, total) =
            group_task_items(items, group_field, sort, request.limit)?;
        TaskListItems::Grouped {
            group_field,
            groups,
            total,
        }
    } else {
        let sort_fields = parse_task_sort_fields(sort)?;
        sort_task_items(&mut items, &sort_fields);
        TaskListItems::Flat {
            items,
            limit: request.limit,
        }
    };
    Ok(TaskListExecution {
        source: request.filters.source,
        items,
    })
}

pub fn collect_shortcut_items_on<E: TaskProviderEnvironment>(
    environment: &E,
    config: &PkmsTaskConfig,
    raw_filters: &[String],
    view: TaskListView,
    clock: TaskClock,
) -> Result<(SourceSelection, Vec<TaskItem>)> {
    let filters = crate::filter::parse_task_filters_on(raw_filters, clock.today)?;
    let source = filters.source;
    let mut items = providers::collect_task_items(environment, &filters, view, clock)?;
    apply_task_filter_criteria_on(config, &mut items, &filters.criteria, clock.today)?;
    sort_task_items(&mut items, &[TaskSortField::Priority]);
    Ok((source, items))
}

pub fn execute_task_agenda<E: TaskProviderEnvironment>(
    environment: &E,
    config: &PkmsTaskConfig,
    request: &AgendaRequest,
) -> Result<AgendaExecution> {
    let mut items =
        providers::collect_task_items(environment, &request.filters, request.view, request.clock)?;
    apply_task_filter_criteria_on(
        config,
        &mut items,
        &request.filters.criteria,
        request.clock.today,
    )?;
    if let Some(days) = request.window.days() {
        retain_agenda_window_task_items_on(&mut items, days, request.clock.today);
    }
    let sort = request.sort.as_deref().unwrap_or("date,priority");
    let sort_fields = parse_task_sort_fields(sort)?;
    sort_task_items(&mut items, &sort_fields);
    Ok(AgendaExecution {
        source: request.filters.source,
        items,
        limit: request.limit,
        window: request.window,
        today: request.clock.today,
    })
}

pub fn apply_task_filter_criteria_on(
    config: &PkmsTaskConfig,
    items: &mut Vec<TaskItem>,
    criteria: &TaskFilterCriteria,
    today: NaiveDate,
) -> Result<()> {
    let before_count = items.len();
    let scope = if criteria.scope.is_empty() {
        None
    } else {
        let workspace = Workspace::load(&config.org)?;
        Some(ResolvedScope::resolve(
            &workspace.graph,
            &config.org.db_root,
            &criteria.scope,
        ))
    };
    let context = TaskFilterContext {
        today,
        scope: scope.as_ref(),
        open_todo_states: &config.task_states.open_states,
        closed_todo_states: &config.task_states.closed_states,
    };
    items.retain(|item| criteria.matches_item(item, &context));
    tracing::debug!(
        before_count,
        after_count = items.len(),
        has_state = criteria.state.is_some(),
        has_tags = criteria.tags.is_some(),
        has_kind = criteria.kind.is_some(),
        has_prio = criteria.prio.is_some(),
        has_date = criteria.date.is_some(),
        has_after = criteria.after.is_some(),
        has_before = criteria.before.is_some(),
        scope_count = criteria.scope.len(),
        has_project = criteria.project.is_some(),
        "applied task filter criteria"
    );

    Ok(())
}
