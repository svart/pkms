use crate::config::ResolvedConfig;
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::org_date::parse_org_date;
use crate::parser::{Heading, find_daily_file_date, strip_org_links};
use crate::tasks::clock::TaskClock;
use crate::tasks::filter::{
    TextFilter, matches_tag_filters, matches_text_filters, matches_type_filters,
};
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
    pub project: Option<String>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub deadline_date: Option<String>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

impl TaskRecord {
    pub fn implicit_daily_file_date(&self) -> Option<&str> {
        if self.scheduled.is_none() && self.deadline.is_none() {
            self.daily_file_date.as_deref()
        } else {
            None
        }
    }

    pub fn effective_date(&self) -> Option<&str> {
        self.scheduled_date
            .as_deref()
            .or(self.deadline_date.as_deref())
            .or_else(|| self.implicit_daily_file_date())
    }

    pub fn has_effective_date(&self, date: &str) -> bool {
        self.scheduled_date.as_deref() == Some(date)
            || self.deadline_date.as_deref() == Some(date)
            || self.implicit_daily_file_date() == Some(date)
    }
}

pub fn collect_todo_records(
    corpus: &Corpus,
    valid_states: &[String],
    state_filters: &[TextFilter],
    tags_filters: &[TextFilter],
    type_filters: &[TextFilter],
) -> Vec<TaskRecord> {
    collect_todo_records_on(
        corpus,
        valid_states,
        state_filters,
        tags_filters,
        type_filters,
        TaskClock::now(),
    )
}

pub fn collect_todo_records_on(
    corpus: &Corpus,
    valid_states: &[String],
    state_filters: &[TextFilter],
    tags_filters: &[TextFilter],
    type_filters: &[TextFilter],
    clock: TaskClock,
) -> Vec<TaskRecord> {
    collect_records(
        corpus,
        |parsed, heading, _is_daily| {
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
        },
        clock,
    )
}

pub fn collect_agenda_records(
    corpus: &Corpus,
    valid_states: &[String],
    closed_states: &[String],
    today: NaiveDate,
    state_filters: &[TextFilter],
    tags_filters: &[TextFilter],
    type_filters: &[TextFilter],
) -> Vec<TaskRecord> {
    collect_agenda_records_on(
        corpus,
        valid_states,
        closed_states,
        TaskClock::at_start_of_day(today),
        state_filters,
        tags_filters,
        type_filters,
    )
}

pub fn collect_agenda_records_on(
    corpus: &Corpus,
    valid_states: &[String],
    closed_states: &[String],
    clock: TaskClock,
    state_filters: &[TextFilter],
    tags_filters: &[TextFilter],
    type_filters: &[TextFilter],
) -> Vec<TaskRecord> {
    collect_records(
        corpus,
        |parsed, heading, is_daily| {
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
        },
        clock,
    )
    .into_iter()
    .map(|mut record| {
        if record.is_daily_file && record.scheduled.is_none() && record.deadline.is_none() {
            record.is_overdue = record.daily_file_date.as_deref().is_some_and(|d| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .ok()
                    .is_some_and(|dt| dt < clock.today)
            });
        }
        record
    })
    .collect()
}

pub fn assign_canonical_ids(config: &ResolvedConfig, graph: &Graph, records: &mut [TaskRecord]) {
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
    corpus: &Corpus,
    mut include_heading: impl FnMut(&crate::parser::ParsedNote, &Heading, bool) -> bool,
    clock: TaskClock,
) -> Vec<TaskRecord> {
    let mut items = Vec::new();
    for result in corpus.results() {
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

            let item_is_overdue = is_overdue_on(heading.deadline.as_ref(), clock)
                || is_overdue_on(heading.scheduled.as_ref(), clock);

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
                project: heading.project.clone().or_else(|| parsed.project.clone()),
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
    state_filters: &[TextFilter],
    tags_filters: &[TextFilter],
    type_filters: &[TextFilter],
) -> bool {
    if !matches_text_filters(heading.todo_state.as_deref(), state_filters) {
        return false;
    }

    let combined_tags = combined_tags(tags);
    if !matches_tag_filters(&combined_tags, tags_filters) {
        return false;
    }

    matches_type_filters(
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

fn extract_date(raw: Option<&String>) -> Option<String> {
    let raw = raw.as_ref()?;
    let parsed = parse_org_date(raw)?;
    Some(parsed.base_date.format("%Y-%m-%d").to_string())
}

fn is_overdue_on(raw: Option<&String>, clock: TaskClock) -> bool {
    let raw = match raw {
        Some(r) => r,
        None => return false,
    };
    let parsed = match parse_org_date(raw) {
        Some(d) => d,
        None => return false,
    };
    let compare_date = parsed.base_date_end.unwrap_or(parsed.base_date);
    if compare_date < clock.today {
        return true;
    }
    if compare_date == clock.today
        && let Some(end_time) = parsed.time_end
    {
        return clock.now > end_time;
    }
    false
}
