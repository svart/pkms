use crate::commands::task_common::{
    Filter, apply_state_filter, apply_tags_filter, apply_type_filter, extract_date, is_overdue,
};
use crate::config::Config;
use crate::graph::Graph;
use crate::parser::{Heading, find_daily_file_date, strip_org_links};
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub has_agenda_tag: bool,
    pub is_daily_file: bool,
    pub daily_file_date: Option<String>,
    pub heading_title: String,
    pub heading_level: usize,
    pub line_number: usize,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub deadline_date: Option<String>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

pub fn collect_todo_records(
    graph: &Graph,
    valid_states: &[String],
    state_filters: &[Filter],
    tags_filters: &[Filter],
    type_filters: &[Filter],
) -> Vec<TaskRecord> {
    collect_records(graph, |parsed, heading, _is_daily| {
        heading
            .todo_state
            .as_ref()
            .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)))
            && apply_common_filters(
                parsed.filetags.iter().chain(heading.tags.iter()),
                heading,
                state_filters,
                tags_filters,
                type_filters,
            )
    })
}

pub fn collect_agenda_records(
    graph: &Graph,
    valid_states: &[String],
    closed_states: &[String],
    today: NaiveDate,
    state_filters: &[Filter],
    tags_filters: &[Filter],
    type_filters: &[Filter],
) -> Vec<TaskRecord> {
    collect_records(graph, |parsed, heading, is_daily| {
        let eligible = heading.scheduled.is_some()
            || heading.deadline.is_some()
            || (is_daily
                && heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s))));

        if !eligible {
            return false;
        }

        if !closed_states.is_empty()
            && let Some(ref todo_state) = heading.todo_state
            && closed_states
                .iter()
                .any(|cs| cs.eq_ignore_ascii_case(todo_state))
        {
            return false;
        }

        apply_common_filters(
            parsed.filetags.iter().chain(heading.tags.iter()),
            heading,
            state_filters,
            tags_filters,
            type_filters,
        )
    })
    .into_iter()
    .map(|mut record| {
        if record.is_daily_file && record.scheduled.is_none() && record.deadline.is_none() {
            record.is_overdue = record.daily_file_date.as_deref().is_some_and(|d| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .ok()
                    .is_some_and(|dt| dt < today)
            });
        }
        record
    })
    .collect()
}

pub fn assign_canonical_ids(config: &Config, graph: &Graph, records: &mut [TaskRecord]) {
    let global_ids: std::collections::HashMap<(String, usize), usize> = graph
        .all_task_entries(config)
        .into_iter()
        .map(|(id, path, line)| ((path, line), id))
        .collect();
    for record in records {
        record.id = global_ids
            .get(&(record.path.clone(), record.line_number))
            .copied()
            .unwrap_or(0);
    }
}

fn collect_records(
    graph: &Graph,
    mut include_heading: impl FnMut(&crate::parser::ParsedNote, &Heading, bool) -> bool,
) -> Vec<TaskRecord> {
    let mut items = Vec::new();
    for result in &graph.results {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path).map(|d| d.format("%Y-%m-%d").to_string());
        let has_agenda = parsed.filetags.iter().any(|t| t == "agenda");
        let primary_uuid = parsed.uuids.first().cloned().unwrap_or_default();
        let note_title = strip_org_links(&parsed.title.clone().unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.display().to_string())
                .unwrap_or_default()
        }));

        for heading in &parsed.headings {
            if !include_heading(parsed, heading, is_daily) {
                continue;
            }

            let item_is_overdue =
                is_overdue(heading.deadline.as_ref()) || is_overdue(heading.scheduled.as_ref());

            items.push(TaskRecord {
                id: 0,
                uuid: primary_uuid.clone(),
                title: note_title.clone(),
                path: path.display().to_string(),
                filetags: parsed.filetags.clone(),
                has_agenda_tag: has_agenda,
                is_daily_file: is_daily,
                daily_file_date: daily_date.clone(),
                heading_title: strip_org_links(&heading.title),
                heading_level: heading.level,
                line_number: heading.line_number,
                todo_state: heading.todo_state.clone(),
                priority: heading.priority,
                scheduled: heading.scheduled.clone(),
                scheduled_date: extract_date(heading.scheduled.as_ref()),
                deadline: heading.deadline.clone(),
                deadline_date: extract_date(heading.deadline.as_ref()),
                is_overdue: item_is_overdue,
                heading_tags: heading.tags.clone(),
            });
        }
    }
    items
}

fn apply_common_filters<'a>(
    tags: impl Iterator<Item = &'a String>,
    heading: &Heading,
    state_filters: &[Filter],
    tags_filters: &[Filter],
    type_filters: &[Filter],
) -> bool {
    if !apply_state_filter(heading.todo_state.as_deref(), state_filters) {
        return false;
    }

    let combined_tags = combined_tags(tags);
    if !apply_tags_filter(&combined_tags, tags_filters) {
        return false;
    }

    apply_type_filter(
        heading.scheduled.is_some(),
        heading.deadline.is_some(),
        type_filters,
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
