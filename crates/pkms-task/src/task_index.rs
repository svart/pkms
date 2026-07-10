use crate::clock::TaskClock;
use crate::config::TaskStateConfig;
use crate::filter::{TextFilter, matches_tag_filters, matches_text_filters, matches_type_filters};
use crate::model::{TaskDateValue, TaskPriority, TaskState};
use crate::projection;
use anyhow::Result;
use chrono::{NaiveDateTime, NaiveTime};
use pkms_org::Graph;
use pkms_org::corpus::Corpus;
use pkms_org::domain::NoteId;
use pkms_org::parser::find_daily_file_date;
use std::cmp::Ordering;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: usize,
    pub uuid: NoteId,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub has_agenda_tag: bool,
    pub is_daily_file: bool,
    pub daily_file_date: Option<TaskDateValue>,
    pub heading_title: String,
    pub heading_level: usize,
    pub line_number: usize,
    pub todo_state: Option<TaskState>,
    pub priority: Option<TaskPriority>,
    pub project: Option<String>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<TaskDateValue>,
    pub deadline: Option<String>,
    pub deadline_date: Option<TaskDateValue>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

#[derive(Debug)]
struct TaskEntry {
    path: String,
    line_number: usize,
    is_open: bool,
    file_order: FileTaskOrder,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FileTaskOrder {
    timestamp: Option<NaiveDateTime>,
    rest_name: String,
    path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CanonicalTaskEntry {
    pub id: usize,
    pub path: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskLocation {
    pub path: String,
    pub line_number: usize,
}

impl TaskRecord {
    pub fn implicit_daily_file_date(&self) -> Option<&str> {
        if self.scheduled.is_none() && self.deadline.is_none() {
            self.daily_file_date.as_ref().map(TaskDateValue::as_str)
        } else {
            None
        }
    }

    pub fn effective_date(&self) -> Option<&str> {
        self.scheduled_date
            .as_ref()
            .map(TaskDateValue::as_str)
            .or_else(|| self.deadline_date.as_ref().map(TaskDateValue::as_str))
            .or_else(|| self.implicit_daily_file_date())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RecordFilters<'a> {
    pub state_filters: &'a [TextFilter],
    pub tags_filters: &'a [TextFilter],
    pub type_filters: &'a [TextFilter],
}

#[derive(Debug, Clone, Copy)]
pub struct TaskRecordQuery<'a> {
    pub valid_states: &'a [String],
    pub closed_states: &'a [String],
    pub filters: RecordFilters<'a>,
    pub clock: TaskClock,
}

impl<'a> TaskRecordQuery<'a> {
    pub fn todo(valid_states: &'a [String], clock: TaskClock) -> Self {
        Self {
            valid_states,
            closed_states: &[],
            filters: RecordFilters::default(),
            clock,
        }
    }

    pub fn agenda(
        valid_states: &'a [String],
        closed_states: &'a [String],
        clock: TaskClock,
    ) -> Self {
        Self {
            valid_states,
            closed_states,
            filters: RecordFilters::default(),
            clock,
        }
    }
}

pub fn collect_todo_records(corpus: &Corpus, query: TaskRecordQuery<'_>) -> Vec<TaskRecord> {
    let TaskRecordQuery {
        valid_states,
        filters,
        clock,
        ..
    } = query;

    projection::collect_todo_records(corpus, valid_states, clock)
        .into_iter()
        .filter(|record| apply_common_filters(record, filters))
        .collect()
}

pub fn collect_agenda_records(corpus: &Corpus, query: TaskRecordQuery<'_>) -> Vec<TaskRecord> {
    let TaskRecordQuery {
        valid_states,
        closed_states,
        filters,
        clock,
    } = query;

    projection::collect_agenda_records(corpus, valid_states, closed_states, clock)
        .into_iter()
        .filter(|record| apply_common_filters(record, filters))
        .collect()
}

pub fn assign_canonical_ids(
    task_states: &TaskStateConfig,
    graph: &Graph,
    records: &mut [TaskRecord],
) {
    let global_ids: std::collections::HashMap<(String, usize), usize> =
        all_task_entries(task_states, graph)
            .into_iter()
            .map(|entry| ((entry.path, entry.line_number), entry.id))
            .collect();
    for record in records {
        record.id = global_ids
            .get(&(record.path.clone(), record.line_number))
            .copied()
            .unwrap_or(0);
    }
}

pub fn all_task_entries(task_states: &TaskStateConfig, graph: &Graph) -> Vec<CanonicalTaskEntry> {
    sorted_task_entries(task_states, graph)
        .into_iter()
        .enumerate()
        .map(|(i, entry)| CanonicalTaskEntry {
            id: i + 1,
            path: entry.path,
            line_number: entry.line_number,
        })
        .collect()
}

pub fn resolve_canonical_task_id(
    task_states: &TaskStateConfig,
    graph: &Graph,
    id: usize,
) -> Result<TaskLocation> {
    let entries = sorted_task_entries(task_states, graph);
    if id == 0 || id > entries.len() {
        anyhow::bail!(
            "No task with canonical ID {}. Valid range is 1-{}",
            id,
            entries.len()
        );
    }
    let entry = &entries[id - 1];
    Ok(TaskLocation {
        path: entry.path.clone(),
        line_number: entry.line_number,
    })
}

fn sorted_task_entries(task_states: &TaskStateConfig, graph: &Graph) -> Vec<TaskEntry> {
    let mut items: Vec<TaskEntry> = Vec::new();

    for result in &graph.results {
        if result.parse_error.is_some() {
            continue;
        }
        let file_order = FileTaskOrder::from_path(&result.path);
        for heading in &result.parsed.headings {
            let is_todo = heading.todo_state.as_ref().is_some_and(|state| {
                task_states
                    .valid_states
                    .iter()
                    .any(|valid| valid.eq_ignore_ascii_case(state))
            });
            let has_dates = heading.scheduled.is_some() || heading.deadline.is_some();
            if !is_todo && !has_dates {
                continue;
            }
            let is_open = match &heading.todo_state {
                Some(state)
                    if task_states
                        .open_states
                        .iter()
                        .any(|open| open.eq_ignore_ascii_case(state)) =>
                {
                    true
                }
                Some(state)
                    if task_states
                        .closed_states
                        .iter()
                        .any(|closed| closed.eq_ignore_ascii_case(state)) =>
                {
                    false
                }
                _ => true,
            };
            items.push(TaskEntry {
                path: result.path.display().to_string(),
                line_number: heading.line_number,
                is_open,
                file_order: file_order.clone(),
            });
        }
    }

    items.sort_by(compare_task_entries);
    items
}

fn compare_task_entries(a: &TaskEntry, b: &TaskEntry) -> Ordering {
    b.is_open
        .cmp(&a.is_open)
        .then_with(|| compare_file_task_order(&a.file_order, &b.file_order))
        .then_with(|| a.line_number.cmp(&b.line_number))
}

impl FileTaskOrder {
    fn from_path(path: &Path) -> Self {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let (timestamp, rest_name) = parse_timestamped_filename(path, filename)
            .map(|(timestamp, rest_name)| (Some(timestamp), rest_name))
            .unwrap_or_else(|| (None, filename.to_string()));

        FileTaskOrder {
            timestamp,
            rest_name,
            path: path.display().to_string(),
        }
    }
}

fn compare_file_task_order(a: &FileTaskOrder, b: &FileTaskOrder) -> Ordering {
    match (&a.timestamp, &b.timestamp) {
        (Some(a_timestamp), Some(b_timestamp)) => b_timestamp
            .cmp(a_timestamp)
            .then_with(|| a.rest_name.cmp(&b.rest_name))
            .then_with(|| a.path.cmp(&b.path)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.path.cmp(&b.path),
    }
}

fn parse_timestamped_filename(path: &Path, filename: &str) -> Option<(NaiveDateTime, String)> {
    let stem = filename.strip_suffix(".org")?;
    if let Some(raw_timestamp) = stem.get(..14)
        && raw_timestamp.chars().all(|c| c.is_ascii_digit())
    {
        let rest_name = match stem.get(14..) {
            Some("") => String::new(),
            Some(rest) => rest.strip_prefix('-')?.to_string(),
            None => return None,
        };
        let timestamp = NaiveDateTime::parse_from_str(raw_timestamp, "%Y%m%d%H%M%S").ok()?;
        return Some((timestamp, rest_name));
    }

    find_daily_file_date(path).map(|date| (date.and_time(NaiveTime::MIN), String::new()))
}

fn apply_common_filters(record: &TaskRecord, filters: RecordFilters<'_>) -> bool {
    if !matches_text_filters(record.todo_state.as_deref(), filters.state_filters) {
        return false;
    }

    let combined_tags = combined_tags(record.filetags.iter().chain(record.heading_tags.iter()));
    if !matches_tag_filters(&combined_tags, filters.tags_filters) {
        return false;
    }

    matches_type_filters(
        record.scheduled.is_some(),
        record.deadline.is_some(),
        filters.type_filters,
    )
}

fn combined_tags<'a>(tags: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for tag in tags {
        if seen.insert(tag.clone()) {
            result.push(tag.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkms_org::corpus::FileScanResult;
    use pkms_org::graph::Graph;
    use pkms_org::parser::{Heading, OrgPriority, OrgTodoState, ParsedNote};
    use std::path::PathBuf;

    fn test_config() -> TaskStateConfig {
        TaskStateConfig {
            valid_states: vec!["TODO".to_string(), "DONE".to_string()],
            open_states: vec!["TODO".to_string()],
            closed_states: vec!["DONE".to_string()],
        }
    }

    fn graph_with_results(results: Vec<FileScanResult>) -> Graph {
        Graph::build(results)
    }

    fn note(path: &str, headings: Vec<Heading>) -> FileScanResult {
        FileScanResult {
            path: PathBuf::from(path),
            parsed: ParsedNote {
                uuids: vec![NoteId::new(format!("{path}-uuid"))],
                title: Some(path.to_string()),
                filetags: Vec::new(),
                project: None,
                categories: Vec::new(),
                aliases: Vec::new(),
                roam_refs: Vec::new(),
                outgoing: Vec::new(),
                headings,
            },
            raw_content: None,
            parse_error: None,
        }
    }

    fn task(
        line_number: usize,
        state: &str,
        priority: Option<char>,
        scheduled: Option<&str>,
        deadline: Option<&str>,
        tags: Vec<&str>,
    ) -> Heading {
        Heading {
            level: 1,
            title: format!("Task {line_number}"),
            todo_state: Some(OrgTodoState::new(state)),
            tags: tags.into_iter().map(str::to_string).collect(),
            uuid: None,
            scheduled: scheduled.map(str::to_string),
            deadline: deadline.map(str::to_string),
            priority: priority.and_then(OrgPriority::from_char),
            project: None,
            line_number,
            outgoing: Vec::new(),
            raw: format!("* {state} Task {line_number}"),
        }
    }

    #[test]
    fn canonical_ids_sort_by_file_timestamp_desc_rest_name_and_line() {
        let graph = graph_with_results(vec![
            note(
                "roam/common/20260524090000-zeta.org",
                vec![
                    task(10, "TODO", None, None, None, Vec::new()),
                    task(20, "TODO", None, None, None, Vec::new()),
                    task(30, "DONE", None, None, None, Vec::new()),
                ],
            ),
            note(
                "roam/common/20260524100000-latest.org",
                vec![
                    task(10, "TODO", Some('C'), None, None, Vec::new()),
                    task(20, "DONE", None, None, None, Vec::new()),
                ],
            ),
            note(
                "roam/common/20260524090000-alpha.org",
                vec![task(10, "TODO", Some('A'), None, None, Vec::new())],
            ),
            note(
                "roam/daily/2026-05-24.org",
                vec![task(10, "TODO", None, None, None, Vec::new())],
            ),
            note(
                "roam/common/tasks.org",
                vec![task(10, "TODO", None, None, None, Vec::new())],
            ),
            note(
                "roam/common/archive.org",
                vec![task(10, "TODO", None, None, None, Vec::new())],
            ),
        ]);

        let entries: Vec<_> = all_task_entries(&test_config(), &graph)
            .into_iter()
            .map(|entry| (entry.path, entry.line_number))
            .collect();

        assert_eq!(
            entries,
            vec![
                ("roam/common/20260524100000-latest.org".to_string(), 10),
                ("roam/common/20260524090000-alpha.org".to_string(), 10),
                ("roam/common/20260524090000-zeta.org".to_string(), 10),
                ("roam/common/20260524090000-zeta.org".to_string(), 20),
                ("roam/daily/2026-05-24.org".to_string(), 10),
                ("roam/common/archive.org".to_string(), 10),
                ("roam/common/tasks.org".to_string(), 10),
                ("roam/common/20260524100000-latest.org".to_string(), 20),
                ("roam/common/20260524090000-zeta.org".to_string(), 30),
            ]
        );
    }

    #[test]
    fn resolves_canonical_task_id_to_task_location() {
        let graph = graph_with_results(vec![note(
            "roam/common/20260524090000-alpha.org",
            vec![task(10, "TODO", None, None, None, Vec::new())],
        )]);

        let location = resolve_canonical_task_id(&test_config(), &graph, 1).unwrap();

        assert_eq!(location.path, "roam/common/20260524090000-alpha.org");
        assert_eq!(location.line_number, 10);
    }

    #[test]
    fn canonical_ids_do_not_change_when_task_properties_change() {
        let graph = graph_with_results(vec![
            note(
                "roam/common/20260524090000-alpha.org",
                vec![task(10, "TODO", None, None, None, Vec::new())],
            ),
            note(
                "roam/common/20260524090000-beta.org",
                vec![task(
                    10,
                    "TODO",
                    Some('A'),
                    Some("<2025-01-01 Wed>"),
                    Some("<2024-01-01 Mon>"),
                    vec!["next"],
                )],
            ),
        ]);

        let entries: Vec<_> = all_task_entries(&test_config(), &graph)
            .into_iter()
            .map(|entry| (entry.path, entry.line_number))
            .collect();

        assert_eq!(
            entries,
            vec![
                ("roam/common/20260524090000-alpha.org".to_string(), 10),
                ("roam/common/20260524090000-beta.org".to_string(), 10),
            ]
        );
    }
}
