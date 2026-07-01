use super::Graph;
use crate::config::ResolvedConfig;
use crate::parser::find_daily_file_date;
use chrono::{NaiveDateTime, NaiveTime};
use std::cmp::Ordering;
use std::path::Path;

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

impl Graph {
    pub fn all_task_entries(&self, config: &ResolvedConfig) -> Vec<CanonicalTaskEntry> {
        let entries = self.sorted_task_entries(config);
        entries
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
        &self,
        config: &ResolvedConfig,
        id: usize,
    ) -> anyhow::Result<TaskLocation> {
        let entries = self.sorted_task_entries(config);
        if id == 0 || id > entries.len() {
            anyhow::bail!(
                "No task with canonical ID {}. Valid range is 1–{}",
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

    fn sorted_task_entries(&self, config: &ResolvedConfig) -> Vec<TaskEntry> {
        let valid_states = config.todo_states();
        let open_states = config.open_todo_states();
        let closed_states = config.closed_todo_states();
        let mut items: Vec<TaskEntry> = Vec::new();

        for result in &self.results {
            if result.parse_error.is_some() {
                continue;
            }
            let file_order = FileTaskOrder::from_path(&result.path);
            for heading in &result.parsed.headings {
                let is_todo = heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)));
                let has_dates = heading.scheduled.is_some() || heading.deadline.is_some();
                if !is_todo && !has_dates {
                    continue;
                }
                let is_open = match &heading.todo_state {
                    Some(s) if open_states.iter().any(|os| os.eq_ignore_ascii_case(s)) => true,
                    Some(s) if closed_states.iter().any(|cs| cs.eq_ignore_ascii_case(s)) => false,
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

#[cfg(test)]
mod tests {
    use crate::config::ResolvedConfig;
    use crate::corpus::FileScanResult;
    use crate::domain::NoteId;
    use crate::graph::{DuplicateInfo, Graph};
    use crate::parser::{Heading, ParsedNote};
    use crate::tasks::model::{TaskPriority, TaskState};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_config() -> ResolvedConfig {
        ResolvedConfig::for_test_db(".")
    }

    fn graph_with_results(results: Vec<FileScanResult>) -> Graph {
        Graph {
            nodes: HashMap::new(),
            path_to_uuid: HashMap::new(),
            title_to_uuid: HashMap::new(),
            alias_to_uuid: HashMap::new(),
            backlinks: HashMap::new(),
            broken_links: Vec::new(),
            parse_errors: Vec::new(),
            skipped_files: Vec::new(),
            duplicates: DuplicateInfo {
                duplicate_uuids: Vec::new(),
                duplicate_titles: Vec::new(),
                missing_titles: Vec::new(),
            },
            heading_uuid_to_primary: HashMap::new(),
            results,
        }
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
            todo_state: Some(TaskState::new(state)),
            tags: tags.into_iter().map(str::to_string).collect(),
            uuid: None,
            scheduled: scheduled.map(str::to_string),
            deadline: deadline.map(str::to_string),
            priority: priority.and_then(TaskPriority::from_char),
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

        let entries: Vec<_> = graph
            .all_task_entries(&test_config())
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

        let location = graph.resolve_canonical_task_id(&test_config(), 1).unwrap();

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

        let entries: Vec<_> = graph
            .all_task_entries(&test_config())
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
