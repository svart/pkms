use crate::config::ResolvedConfig;
use crate::parser::find_daily_file_date;
use crate::tasks::clock::TaskClock;
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDate, TaskItem, TaskSourceKind, TaskStatus};
use crate::tasks::pkms_edit;
use crate::tasks::task_index::{
    TaskRecord, assign_canonical_ids, collect_agenda_records_on, collect_todo_records_on,
};
use crate::workspace::Workspace;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum PkmsInboxTarget {
    Note(PathBuf),
    Daily { path: PathBuf },
}

pub fn list_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    list_items_on(config, TaskClock::now())
}

pub fn list_items_on(config: &ResolvedConfig, clock: TaskClock) -> Result<Vec<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let valid_states = config.todo_states();
    let no_filters = Vec::new();
    let mut records = collect_todo_records_on(
        &workspace.corpus,
        &valid_states,
        &no_filters,
        &no_filters,
        &no_filters,
        clock,
    );
    assign_canonical_ids(config, &workspace.graph, &mut records);
    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
}

pub fn collect_inbox_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    collect_inbox_items_on(config, TaskClock::now())
}

pub fn collect_inbox_items_on(config: &ResolvedConfig, clock: TaskClock) -> Result<Vec<TaskItem>> {
    let target = resolve_inbox_target_on(config, false, clock.today)?;
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;
    let valid_states = config.todo_states();
    let mut records =
        collect_todo_records_on(&workspace.corpus, &valid_states, &[], &[], &[], clock);
    assign_canonical_ids(config, graph, &mut records);

    let records: Vec<_> = match target {
        PkmsInboxTarget::Note(path) => records
            .into_iter()
            .filter(|record| record.path == path.display().to_string())
            .collect(),
        PkmsInboxTarget::Daily { path } => {
            let section = pkms_edit::inbox_section_range(&path)?;
            records
                .into_iter()
                .filter(|record| {
                    record.path == path.display().to_string()
                        && section.is_some_and(|(start, end)| {
                            record.line_number > start && record.line_number < end
                        })
                })
                .collect()
        }
    };

    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
}

pub fn resolve_inbox_target(
    config: &ResolvedConfig,
    create_daily: bool,
) -> Result<PkmsInboxTarget> {
    resolve_inbox_target_on(config, create_daily, TaskClock::now().today)
}

pub fn resolve_inbox_target_on(
    config: &ResolvedConfig,
    create_daily: bool,
    today: NaiveDate,
) -> Result<PkmsInboxTarget> {
    let target = config.task_inbox()?;
    if target.eq_ignore_ascii_case("daily") {
        return resolve_daily_inbox_target(config, create_daily, today);
    }

    let graph = crate::graph::Graph::load(config)?;
    if let Some(node) = graph.find_node(target) {
        return Ok(PkmsInboxTarget::Note(node.path.clone()));
    }

    let configured = PathBuf::from(target);
    let candidates = if configured.is_absolute() {
        vec![configured]
    } else {
        vec![config.resolved_db_root().join(&configured), configured]
    };

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(PkmsInboxTarget::Note)
        .ok_or_else(|| anyhow::anyhow!("PKMS task inbox note not found: {target}"))
}

pub fn resolve_note_task_target(config: &ResolvedConfig, target: &str) -> Result<PkmsInboxTarget> {
    let graph = crate::graph::Graph::load(config)?;
    if let Some(node) = graph.find_node(target) {
        return Ok(PkmsInboxTarget::Note(node.path.clone()));
    }

    let configured = PathBuf::from(target);
    let candidates = if configured.is_absolute() {
        vec![configured]
    } else {
        vec![config.resolved_db_root().join(&configured), configured]
    };

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(PkmsInboxTarget::Note)
        .ok_or_else(|| anyhow::anyhow!("PKMS task target note not found: {target}"))
}

fn resolve_daily_inbox_target(
    config: &ResolvedConfig,
    create: bool,
    today: NaiveDate,
) -> Result<PkmsInboxTarget> {
    let configured_path = config
        .resolve_daily_notes_dir()
        .join(format!("{today}.org"));
    if config.daily_notes_dir.is_some() || configured_path.exists() {
        if create {
            ensure_daily_note_exists(&configured_path, today)?;
        }
        return Ok(PkmsInboxTarget::Daily {
            path: configured_path,
        });
    }

    let graph = crate::graph::Graph::load(config)?;
    if let Some(result) = graph
        .results
        .iter()
        .find(|result| find_daily_file_date(&result.path) == Some(today))
    {
        return Ok(PkmsInboxTarget::Daily {
            path: result.path.clone(),
        });
    }

    if !create {
        return Ok(PkmsInboxTarget::Daily {
            path: configured_path,
        });
    }

    ensure_daily_note_exists(&configured_path, today)?;
    Ok(PkmsInboxTarget::Daily {
        path: configured_path,
    })
}

fn ensure_daily_note_exists(path: &Path, today: chrono::NaiveDate) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create daily note directory: {}",
                parent.display()
            )
        })?;
    }
    if !path.exists() {
        let title = today.format("%Y-%m-%d").to_string();
        let uuid = uuid::Uuid::new_v4();
        std::fs::write(
            path,
            format!(":PROPERTIES:\n:ID:       {uuid}\n:END:\n#+title: {title}\n\n"),
        )
        .with_context(|| format!("Failed to create daily note: {}", path.display()))?;
    }
    Ok(())
}

pub fn append_inbox_entry(target: &PkmsInboxTarget, entry: &str) -> Result<(PathBuf, usize)> {
    match target {
        PkmsInboxTarget::Note(path) => {
            pkms_edit::append_org_entry(path, entry).map(|line| (path.clone(), line))
        }
        PkmsInboxTarget::Daily { path } => {
            pkms_edit::append_daily_inbox_entry(path, entry).map(|line| (path.clone(), line))
        }
    }
}

pub fn heading_level_at(path: &Path, line_number: usize) -> Result<usize> {
    pkms_edit::heading_level_at(path, line_number)
}

pub fn append_child_entry(
    path: &Path,
    parent_line_number: usize,
    entry: &str,
) -> Result<(PathBuf, usize)> {
    pkms_edit::append_child_org_entry(path, parent_line_number, entry)
        .map(|line| (path.to_path_buf(), line))
}

pub fn move_subtree_to_dependency(
    source_path: &Path,
    source_line_number: usize,
    target_path: &Path,
    target_line_number: usize,
) -> Result<(PathBuf, usize)> {
    pkms_edit::move_org_subtree(
        source_path,
        source_line_number,
        target_path,
        target_line_number,
    )
    .map(|line| (target_path.to_path_buf(), line))
}

pub fn remove_subtree_dependency(
    path: &Path,
    source_line_number: usize,
    parent_line_number: usize,
) -> Result<(PathBuf, usize)> {
    pkms_edit::remove_org_subtree_dependency(path, source_line_number, parent_line_number)
        .map(|line| (path.to_path_buf(), line))
}

pub fn find_task_item(
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
) -> Result<Option<TaskItem>> {
    find_task_item_on(config, path, line_number, TaskClock::now())
}

pub fn find_task_item_on(
    config: &ResolvedConfig,
    path: &Path,
    line_number: usize,
    clock: TaskClock,
) -> Result<Option<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let graph = &workspace.graph;
    let valid_states = config.todo_states();
    let mut records =
        collect_todo_records_on(&workspace.corpus, &valid_states, &[], &[], &[], clock);
    assign_canonical_ids(config, graph, &mut records);
    Ok(records
        .into_iter()
        .find(|record| {
            record.path == path.display().to_string() && record.line_number == line_number
        })
        .map(|record| record_to_task_item(config, record)))
}

pub fn agenda_items(config: &ResolvedConfig) -> Result<Vec<TaskItem>> {
    agenda_items_for(config, false, false, false, false)
}

pub fn agenda_items_for(
    config: &ResolvedConfig,
    today_only: bool,
    week: bool,
    overdue: bool,
    upcoming: bool,
) -> Result<Vec<TaskItem>> {
    agenda_items_for_on(
        config,
        today_only,
        week,
        overdue,
        upcoming,
        TaskClock::now().today,
    )
}

pub fn agenda_items_for_on(
    config: &ResolvedConfig,
    today_only: bool,
    week: bool,
    overdue: bool,
    upcoming: bool,
    today: NaiveDate,
) -> Result<Vec<TaskItem>> {
    agenda_items_for_clock(
        config,
        today_only,
        week,
        overdue,
        upcoming,
        TaskClock::at_start_of_day(today),
    )
}

pub fn agenda_items_for_clock(
    config: &ResolvedConfig,
    today_only: bool,
    week: bool,
    overdue: bool,
    upcoming: bool,
    clock: TaskClock,
) -> Result<Vec<TaskItem>> {
    let workspace = Workspace::load(config)?;
    let valid_states = config.todo_states();
    let closed_states = config.closed_todo_states();
    let no_filters = Vec::new();
    let mut records = collect_agenda_records_on(
        &workspace.corpus,
        &valid_states,
        &closed_states,
        clock,
        &no_filters,
        &no_filters,
        &no_filters,
    );

    if week {
        let cutoff = clock.today + chrono::Duration::days(7);
        records.retain(|item| item_date(item).is_some_and(|date| date <= cutoff));
    } else if today_only {
        records.retain(|item| item_date(item).is_some_and(|date| date == clock.today));
    }

    if overdue {
        records.retain(|item| item.is_overdue);
    }

    if upcoming {
        records.retain(|item| {
            !item.is_overdue && item_date(item).is_some_and(|date| date > clock.today)
        });
    }

    assign_canonical_ids(config, &workspace.graph, &mut records);
    Ok(records
        .into_iter()
        .map(|record| record_to_task_item(config, record))
        .collect())
}

pub fn record_to_task_item(config: &ResolvedConfig, record: TaskRecord) -> TaskItem {
    let id = TaskId::Pkms(record.id);
    let source_id = id.source_id();
    let display_id = id.display_id();
    let status = pkms_status(config, record.todo_state.as_deref());
    TaskItem {
        id,
        display_id,
        source: TaskSourceKind::Pkms,
        source_id,
        title: record.heading_title,
        body: None,
        status,
        state: record.todo_state,
        priority: record.priority.map(|p| p.to_string()),
        scheduled: record.scheduled.map(|raw| TaskDate {
            raw,
            date: record.scheduled_date,
        }),
        deadline: record.deadline.map(|raw| TaskDate {
            raw,
            date: record.deadline_date,
        }),
        tags: combine_tags(&record.filetags, &record.heading_tags),
        project: record.project.clone(),
        project_id: record.project,
        note_title: Some(record.title),
        note_uuid: Some(record.uuid),
        path: Some(PathBuf::from(record.path)),
        has_agenda_tag: Some(record.has_agenda_tag),
        is_daily_file: record.is_daily_file,
        daily_file_date: record.daily_file_date,
        heading_level: Some(record.heading_level),
        line_number: Some(record.line_number),
        url: None,
        is_overdue: record.is_overdue,
    }
}

fn pkms_status(config: &ResolvedConfig, todo_state: Option<&str>) -> TaskStatus {
    let Some(todo_state) = todo_state else {
        return TaskStatus::Unknown;
    };

    if config
        .closed_todo_states()
        .iter()
        .any(|state| state.eq_ignore_ascii_case(todo_state))
    {
        return TaskStatus::Done;
    }

    if config
        .open_todo_states()
        .iter()
        .any(|state| state.eq_ignore_ascii_case(todo_state))
    {
        return TaskStatus::Open;
    }

    TaskStatus::Unknown
}

fn combine_tags(filetags: &[String], heading_tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tags = Vec::new();
    for tag in filetags.iter().chain(heading_tags.iter()) {
        if seen.insert(tag.clone()) {
            tags.push(tag.clone());
        }
    }
    tags
}

fn item_date(item: &TaskRecord) -> Option<NaiveDate> {
    item.effective_date()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn config() -> ResolvedConfig {
        Config {
            db_root: None,
            new_notes_dir: None,
            daily_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: Some(crate::config::AgendaConfig {
                open_todo_states: vec!["TODO".to_string(), "WAITING".to_string()],
                closed_todo_states: vec!["DONE".to_string(), "CANCELED".to_string()],
            }),
            todoist: None,
            ssh: None,
        }
        .resolve(Some(std::path::PathBuf::from(".")))
        .unwrap()
    }

    fn record() -> TaskRecord {
        TaskRecord {
            id: 7,
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            title: "Note A".to_string(),
            path: "/tmp/a.org".to_string(),
            filetags: vec!["agenda".to_string(), "work".to_string()],
            has_agenda_tag: true,
            is_daily_file: false,
            daily_file_date: None,
            heading_title: "Call supplier".to_string(),
            heading_level: 1,
            line_number: 12,
            todo_state: Some("todo".to_string()),
            priority: Some('A'),
            project: Some("Work".to_string()),
            scheduled: Some("<2026-05-23 Sat>".to_string()),
            scheduled_date: Some("2026-05-23".to_string()),
            deadline: None,
            deadline_date: None,
            is_overdue: false,
            heading_tags: vec!["work".to_string(), "phone".to_string()],
        }
    }

    #[test]
    fn converts_pkms_task_record_to_source_neutral_item() {
        let item = record_to_task_item(&config(), record());
        assert_eq!(item.id, TaskId::Pkms(7));
        assert_eq!(item.display_id, "p7");
        assert_eq!(item.source, TaskSourceKind::Pkms);
        assert_eq!(item.status, TaskStatus::Open);
        assert_eq!(item.title, "Call supplier");
        assert_eq!(item.note_title.as_deref(), Some("Note A"));
        assert_eq!(item.has_agenda_tag, Some(true));
        assert!(!item.is_daily_file);
        assert_eq!(item.heading_level, Some(1));
        assert_eq!(item.project.as_deref(), Some("Work"));
        assert_eq!(item.project_id.as_deref(), Some("Work"));
        assert_eq!(item.tags, vec!["agenda", "work", "phone"]);
        assert_eq!(item.scheduled.unwrap().date.as_deref(), Some("2026-05-23"));
    }

    #[test]
    fn closed_state_maps_to_done_case_insensitively() {
        let mut record = record();
        record.todo_state = Some("done".to_string());
        let item = record_to_task_item(&config(), record);
        assert_eq!(item.status, TaskStatus::Done);
    }
}
