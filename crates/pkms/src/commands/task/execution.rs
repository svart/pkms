use super::{plan, providers};
use crate::commands::task_common::{
    AgendaWindow, RowSeparatorMode, TaskGroupField, group_task_items, parse_task_sort_fields,
    retain_agenda_window_task_items_on, sort_task_items,
};
use crate::config::{ResolvedConfig, TaskCommandConfig};
use crate::output::Column;
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{SourceSelection, TaskFilterContext, TaskFilterCriteria};
use crate::tasks::model::TaskItem;
use crate::tasks::provider::TaskListView;
use crate::tasks::scope::ResolvedScope;
use anyhow::Result;
use chrono::NaiveDate;
use pkms_org::Workspace;
use std::collections::BTreeMap;

pub(super) struct TaskListExecution {
    pub(super) source: SourceSelection,
    pub(super) items: TaskListItems,
    pub(super) row_separators: RowSeparatorMode,
    pub(super) columns: Option<Vec<Column>>,
}

pub(super) enum TaskListItems {
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

pub(super) struct AgendaExecution {
    pub(super) source: SourceSelection,
    pub(super) items: Vec<TaskItem>,
    pub(super) limit: Option<usize>,
    pub(super) window: AgendaWindow,
    pub(super) row_separators: RowSeparatorMode,
    pub(super) columns: Option<Vec<Column>>,
    pub(super) today: NaiveDate,
}

pub(super) fn execute_task_list(
    config: &ResolvedConfig,
    request: &plan::TaskListRequest,
) -> Result<TaskListExecution> {
    let task_config = config.task_command_config();
    let mut items =
        providers::collect_task_items(config, &request.filters, TaskListView::All, request.clock)?;
    let mut criteria = request.filters.criteria.clone();
    criteria.scope = request.scope.clone();
    apply_task_filter_criteria_on(&task_config, &mut items, &criteria, request.clock.today)?;
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
        row_separators: request.row_separators,
        columns: request.columns.clone(),
    })
}

pub(super) fn collect_shortcut_items_on(
    config: &ResolvedConfig,
    raw_filters: &[String],
    kind: plan::ShortcutKind,
    clock: TaskClock,
) -> Result<(SourceSelection, Vec<TaskItem>)> {
    let task_config = config.task_command_config();
    let filters = crate::tasks::filter::parse_task_filters_on(raw_filters, clock.today)?;
    let source = filters.source;
    let mut items =
        providers::collect_task_items(config, &filters, plan::shortcut_task_view(kind), clock)?;
    apply_task_filter_criteria_on(&task_config, &mut items, &filters.criteria, clock.today)?;
    Ok((source, items))
}

pub(super) fn execute_task_agenda(
    config: &ResolvedConfig,
    request: &plan::AgendaRequest,
) -> Result<AgendaExecution> {
    let task_config = config.task_command_config();
    let mut items =
        providers::collect_task_items(config, &request.filters, request.view, request.clock)?;
    apply_task_filter_criteria_on(
        &task_config,
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
        row_separators: request.row_separators,
        columns: request.columns.clone(),
        today: request.clock.today,
    })
}

fn apply_task_filter_criteria_on(
    config: &TaskCommandConfig,
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
