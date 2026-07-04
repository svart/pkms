use super::plan;
use crate::commands::task_common::{AgendaWindow, RowSeparatorMode};
use crate::config::ResolvedConfig;
use crate::output::Column;
use anyhow::Result;
use chrono::NaiveDate;
use pkms_task::clock::TaskClock;
use pkms_task::filter::SourceSelection;
use pkms_task::model::TaskItem;

pub(super) use pkms_task::execution::TaskListItems;

pub(super) struct TaskListExecution {
    pub(super) source: SourceSelection,
    pub(super) items: TaskListItems,
    pub(super) row_separators: RowSeparatorMode,
    pub(super) columns: Option<Vec<Column>>,
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
    let task_config = config.pkms_task_config();
    let output = pkms_task::execution::execute_task_list(
        config,
        &task_config,
        &pkms_task::execution::TaskListRequest {
            filters: request.filters.clone(),
            scope: request.scope.clone(),
            sort: request.sort.clone(),
            limit: request.limit,
            group: request.group.clone(),
            clock: request.clock,
        },
    )?;
    Ok(TaskListExecution {
        source: output.source,
        items: output.items,
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
    pkms_task::execution::collect_shortcut_items_on(
        config,
        &config.pkms_task_config(),
        raw_filters,
        plan::shortcut_task_view(kind),
        clock,
    )
}

pub(super) fn execute_task_agenda(
    config: &ResolvedConfig,
    request: &plan::AgendaRequest,
) -> Result<AgendaExecution> {
    let task_config = config.pkms_task_config();
    let output = pkms_task::execution::execute_task_agenda(
        config,
        &task_config,
        &pkms_task::execution::AgendaRequest {
            filters: request.filters.clone(),
            sort: request.sort.clone(),
            limit: request.limit,
            window: request.window,
            clock: request.clock,
            view: request.view,
        },
    )?;
    Ok(AgendaExecution {
        source: output.source,
        items: output.items,
        limit: output.limit,
        window: output.window,
        row_separators: request.row_separators,
        columns: request.columns.clone(),
        today: output.today,
    })
}
