use crate::commands::task_index::TaskRecord;
use crate::config::ResolvedConfig;
use crate::tasks::id::TaskId;
use crate::tasks::model::{TaskDate, TaskItem, TaskSourceKind, TaskStatus};
use std::collections::HashSet;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn config() -> ResolvedConfig {
        Config {
            db_root: None,
            new_notes_dir: None,
            ignore_patterns: None,
            columns: None,
            tasks: None,
            agenda: Some(crate::config::AgendaConfig {
                open_todo_states: vec!["TODO".to_string(), "WAITING".to_string()],
                closed_todo_states: vec!["DONE".to_string(), "CANCELED".to_string()],
            }),
            todoist: None,
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
