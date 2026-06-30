use super::{plan, providers, render};
use crate::commands::task_common::{
    AgendaWindow, RowSeparatorMode, date_in_agenda_window, parse_task_sort_fields,
    validate_task_group_field,
};
use crate::config::ResolvedConfig;
use crate::output::Column;
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{SourceSelection, TaskFilterContext, TaskFilterCriteria};
use crate::tasks::model::TaskItem;
use crate::tasks::provider::TaskListView;
use crate::tasks::scope::ResolvedScope;
use crate::workspace::Workspace;
use anyhow::Result;
use chrono::NaiveDate;
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
        group_field: String,
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
    let mut items =
        providers::collect_task_items(config, &request.filters, TaskListView::All, request.clock)?;
    let mut criteria = request.filters.criteria.clone();
    criteria.scope = request.scope.clone();
    apply_task_filter_criteria_on(config, &mut items, &criteria, request.clock.today)?;
    let sort = request.sort.as_deref().unwrap_or("date,priority");
    let items = if let Some(group_field) = &request.group {
        let (groups, total) = group_task_items(items, group_field, sort, request.limit)?;
        TaskListItems::Grouped {
            group_field: group_field.clone(),
            groups,
            total,
        }
    } else {
        sort_task_items(&mut items, sort)?;
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
    let filters = crate::tasks::filter::parse_task_filters_on(raw_filters, clock.today)?;
    let source = filters.source;
    let mut items =
        providers::collect_task_items(config, &filters, plan::shortcut_task_view(kind), clock)?;
    apply_task_filter_criteria_on(config, &mut items, &filters.criteria, clock.today)?;
    Ok((source, items))
}

pub(super) fn execute_task_agenda(
    config: &ResolvedConfig,
    request: &plan::AgendaRequest,
) -> Result<AgendaExecution> {
    let mut items =
        providers::collect_task_items(config, &request.filters, request.view, request.clock)?;
    apply_task_filter_criteria_on(
        config,
        &mut items,
        &request.filters.criteria,
        request.clock.today,
    )?;
    if let Some(days) = request.window.days() {
        retain_agenda_window_task_items_on(&mut items, days, request.clock.today);
    }
    sort_task_items(
        &mut items,
        request.sort.as_deref().unwrap_or("date,priority"),
    )?;
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

fn group_task_items(
    items: Vec<TaskItem>,
    group_field: &str,
    sort: &str,
    limit: Option<usize>,
) -> Result<(BTreeMap<String, Vec<TaskItem>>, usize)> {
    validate_task_group_field(group_field)?;
    let mut groups: BTreeMap<String, Vec<TaskItem>> = BTreeMap::new();
    for item in items {
        groups
            .entry(task_group_key(&item, group_field))
            .or_default()
            .push(item);
    }

    let total = groups.values().map(Vec::len).sum();
    for group_items in groups.values_mut() {
        sort_task_items(group_items, sort)?;
        if let Some(limit) = limit {
            group_items.truncate(limit);
        }
    }
    Ok((groups, total))
}

fn task_group_key(item: &TaskItem, group_field: &str) -> String {
    match group_field {
        "state" => item.state.as_deref().unwrap_or("NONE").to_string(),
        "file" => item.note_title.clone().unwrap_or_default(),
        "priority" => match item.priority_char() {
            Some('A') => "Priority A".to_string(),
            Some('B') => "Priority B".to_string(),
            Some('C') => "Priority C".to_string(),
            _ => "No Priority".to_string(),
        },
        _ => unreachable!("task group field was validated"),
    }
}

fn apply_task_filter_criteria_on(
    config: &ResolvedConfig,
    items: &mut Vec<TaskItem>,
    criteria: &TaskFilterCriteria,
    today: NaiveDate,
) -> Result<()> {
    let before_count = items.len();
    let scope = if criteria.scope.is_empty() {
        None
    } else {
        let workspace = Workspace::load(config)?;
        Some(ResolvedScope::resolve(
            &workspace.graph,
            config.resolved_db_root(),
            &criteria.scope,
        ))
    };
    let open_todo_states = config.open_todo_states();
    let closed_todo_states = config.closed_todo_states();
    let context = TaskFilterContext {
        today,
        scope: scope.as_ref(),
        open_todo_states: &open_todo_states,
        closed_todo_states: &closed_todo_states,
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

pub(super) fn retain_upcoming_task_items_on(
    items: &mut Vec<TaskItem>,
    days: i64,
    today: NaiveDate,
) {
    let cutoff = today + chrono::Duration::days(days);
    items.retain(|item| {
        item.effective_date()
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .is_some_and(|date| date > today && date <= cutoff)
    });
}

fn retain_agenda_window_task_items_on(items: &mut Vec<TaskItem>, days: i64, today: NaiveDate) {
    items.retain(|item| {
        item.is_overdue_on(today)
            || item
                .dates()
                .into_iter()
                .any(|date| date_in_agenda_window(date, today, days))
    });
}

pub(super) fn sort_task_items(items: &mut [TaskItem], sort: &str) -> Result<()> {
    let fields = parse_task_sort_fields(sort)?;
    items.sort_by(|a, b| {
        for field in &fields {
            let ord = match *field {
                "priority" => a.priority_sort_value().cmp(&b.priority_sort_value()),
                "date" => a.effective_date().cmp(&b.effective_date()),
                "scheduled" => a.scheduled_date_str().cmp(&b.scheduled_date_str()),
                "deadline" => a.deadline_date_str().cmp(&b.deadline_date_str()),
                "file" => a.note_title.cmp(&b.note_title),
                "source" => render::source_name(a).cmp(render::source_name(b)),
                "state" => a.state.cmp(&b.state),
                "task" | "title" => a.title.cmp(&b.title),
                "project" => a.project.cmp(&b.project),
                _ => std::cmp::Ordering::Equal,
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        render::source_name(a)
            .cmp(render::source_name(b))
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(())
}
