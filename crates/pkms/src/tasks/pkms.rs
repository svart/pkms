use crate::config::ResolvedConfig;
use crate::tasks::clock::TaskClock;
use crate::tasks::model::TaskItem;
use anyhow::Result;
use chrono::NaiveDate;
use std::path::Path;

pub use pkms_task::pkms::{
    AgendaView, PkmsInboxTarget, TaskLocation, append_child_entry, append_inbox_entry,
    heading_level_at, move_subtree_to_dependency, record_to_task_item, remove_subtree_dependency,
};

pub fn list_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::list_items(&config.pkms_task_config())
}

pub fn list_items_on(config: &ResolvedConfig, clock: TaskClock) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::list_items_on(&config.pkms_task_config(), clock)
}

pub fn collect_inbox_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::collect_inbox_items(&config.pkms_task_config())
}

pub fn collect_inbox_items_on(config: &ResolvedConfig, clock: TaskClock) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::collect_inbox_items_on(&config.pkms_task_config(), clock)
}

pub fn resolve_inbox_target(
    config: &ResolvedConfig,
    create_daily: bool,
) -> Result<PkmsInboxTarget> {
    pkms_task::pkms::resolve_inbox_target(&config.pkms_task_config(), create_daily)
}

pub fn resolve_inbox_target_on(
    config: &ResolvedConfig,
    create_daily: bool,
    today: NaiveDate,
) -> Result<PkmsInboxTarget> {
    pkms_task::pkms::resolve_inbox_target_on(&config.pkms_task_config(), create_daily, today)
}

pub fn resolve_note_task_target(config: &ResolvedConfig, target: &str) -> Result<PkmsInboxTarget> {
    pkms_task::pkms::resolve_note_task_target(&config.pkms_task_config(), target)
}

pub fn find_task_item(
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
) -> Result<Option<TaskItem>> {
    pkms_task::pkms::find_task_item(&config.pkms_task_config(), path, line_number)
}

pub fn find_task_item_on(
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
    clock: TaskClock,
) -> Result<Option<TaskItem>> {
    pkms_task::pkms::find_task_item_on(&config.pkms_task_config(), path, line_number, clock)
}

pub fn agenda_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::agenda_items(&config.pkms_task_config())
}

pub fn agenda_items_for(config: &ResolvedConfig, view: AgendaView) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::agenda_items_for(&config.pkms_task_config(), view)
}

pub fn agenda_items_for_on(
    config: &ResolvedConfig,
    view: AgendaView,
    today: NaiveDate,
) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::agenda_items_for_on(&config.pkms_task_config(), view, today)
}

pub fn agenda_items_for_clock(
    config: &ResolvedConfig,
    view: AgendaView,
    clock: TaskClock,
) -> Result<Vec<TaskItem>> {
    pkms_task::pkms::agenda_items_for_clock(&config.pkms_task_config(), view, clock)
}
