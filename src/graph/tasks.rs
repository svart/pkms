use super::Graph;
use crate::config::ResolvedConfig;
use crate::org_date::parse_org_date;
use crate::util::priority_value;
use chrono::NaiveDate;
use std::cmp::Ordering;

#[derive(Debug)]
struct TaskEntry {
    path: String,
    line_number: usize,
    priority: Option<char>,
    is_open: bool,
    deadline_date: Option<NaiveDate>,
    scheduled_date: Option<NaiveDate>,
}

impl Graph {
    pub fn all_task_entries(&self, config: &ResolvedConfig) -> Vec<(usize, String, usize)> {
        let entries = self.sorted_task_entries(config);
        entries
            .into_iter()
            .enumerate()
            .map(|(i, entry)| (i + 1, entry.path, entry.line_number))
            .collect()
    }

    pub fn resolve_canonical_task_id(
        &self,
        config: &ResolvedConfig,
        id: usize,
    ) -> anyhow::Result<(String, usize)> {
        let entries = self.sorted_task_entries(config);
        if id == 0 || id > entries.len() {
            anyhow::bail!(
                "No task with canonical ID {}. Valid range is 1–{}",
                id,
                entries.len()
            );
        }
        let entry = &entries[id - 1];
        Ok((entry.path.clone(), entry.line_number))
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
                    priority: heading.priority,
                    is_open,
                    deadline_date: task_date(heading.deadline.as_deref()),
                    scheduled_date: task_date(heading.scheduled.as_deref()),
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
        .then_with(|| priority_sort_value(a).cmp(&priority_sort_value(b)))
        .then_with(|| cmp_optional_date_none_last(&a.deadline_date, &b.deadline_date))
        .then_with(|| cmp_optional_date_none_last(&a.scheduled_date, &b.scheduled_date))
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| a.line_number.cmp(&b.line_number))
}

fn priority_sort_value(entry: &TaskEntry) -> u8 {
    entry.priority.map(priority_value).unwrap_or(3)
}

fn task_date(raw: Option<&str>) -> Option<NaiveDate> {
    raw.and_then(parse_org_date).map(|date| date.base_date)
}

fn cmp_optional_date_none_last(a: &Option<NaiveDate>, b: &Option<NaiveDate>) -> Ordering {
    a.is_none().cmp(&b.is_none()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use crate::config::ResolvedConfig;
    use crate::corpus::FileScanResult;
    use crate::graph::{DuplicateInfo, Graph};
    use crate::parser::{Heading, ParsedNote};
    use crate::util::priority_value;
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
                uuids: vec![format!("{path}-uuid")],
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
            todo_state: Some(state.to_string()),
            tags: tags.into_iter().map(str::to_string).collect(),
            uuid: None,
            scheduled: scheduled.map(str::to_string),
            deadline: deadline.map(str::to_string),
            priority,
            project: None,
            line_number,
            outgoing: Vec::new(),
            raw: format!("* {state} Task {line_number}"),
        }
    }

    #[test]
    fn test_canonical_priority_ordering() {
        assert!(priority_value('A') < priority_value('B'));
        assert!(priority_value('B') < priority_value('C'));
        assert!(priority_value('C') < priority_value('D'));
    }

    #[test]
    fn test_canonical_priority_values() {
        assert_eq!(priority_value('A'), 0);
        assert_eq!(priority_value('B'), 1);
        assert_eq!(priority_value('C'), 2);
        assert_eq!(priority_value('Z'), 3);
        assert_eq!(priority_value('X'), 3);
    }

    #[test]
    fn canonical_ids_sort_by_open_priority_deadline_scheduled_path_and_line() {
        let graph = graph_with_results(vec![
            note(
                "b.org",
                vec![
                    task(10, "TODO", None, None, None, Vec::new()),
                    task(
                        20,
                        "TODO",
                        Some('B'),
                        Some("<2025-02-01 Sat>"),
                        None,
                        Vec::new(),
                    ),
                    task(
                        30,
                        "TODO",
                        Some('B'),
                        None,
                        Some("<2025-05-01 Thu>"),
                        Vec::new(),
                    ),
                    task(
                        40,
                        "TODO",
                        Some('A'),
                        None,
                        Some("<2026-12-31 Thu>"),
                        Vec::new(),
                    ),
                    task(
                        50,
                        "DONE",
                        Some('A'),
                        None,
                        Some("<2020-01-01 Wed>"),
                        Vec::new(),
                    ),
                ],
            ),
            note(
                "a.org",
                vec![
                    task(
                        10,
                        "TODO",
                        Some('B'),
                        None,
                        Some("<2025-04-01 Tue>"),
                        Vec::new(),
                    ),
                    task(
                        20,
                        "TODO",
                        Some('B'),
                        Some("<2025-01-01 Wed>"),
                        None,
                        Vec::new(),
                    ),
                    task(30, "TODO", Some('B'), None, None, Vec::new()),
                ],
            ),
        ]);

        let entries: Vec<_> = graph
            .all_task_entries(&test_config())
            .into_iter()
            .map(|(_, path, line)| (path, line))
            .collect();

        assert_eq!(
            entries,
            vec![
                ("b.org".to_string(), 40),
                ("a.org".to_string(), 10),
                ("b.org".to_string(), 30),
                ("a.org".to_string(), 20),
                ("b.org".to_string(), 20),
                ("a.org".to_string(), 30),
                ("b.org".to_string(), 10),
                ("b.org".to_string(), 50),
            ]
        );
    }

    #[test]
    fn canonical_ids_do_not_special_case_tags() {
        let graph = graph_with_results(vec![
            note(
                "b.org",
                vec![task(10, "TODO", Some('B'), None, None, vec!["next"])],
            ),
            note(
                "a.org",
                vec![task(10, "TODO", Some('B'), None, None, Vec::new())],
            ),
        ]);

        let entries: Vec<_> = graph
            .all_task_entries(&test_config())
            .into_iter()
            .map(|(_, path, line)| (path, line))
            .collect();

        assert_eq!(
            entries,
            vec![("a.org".to_string(), 10), ("b.org".to_string(), 10)]
        );
    }
}
