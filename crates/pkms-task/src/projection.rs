use crate::clock::TaskClock;
use crate::model::{TaskDateValue, TaskPriority, TaskState};
use crate::task_index::TaskRecord;
use pkms_org::corpus::Corpus;
use pkms_org::org_date::parse_org_date;
use pkms_org::parser::{Heading, OrgPriority, OrgTodoState, find_daily_file_date, strip_org_links};

#[derive(Debug, Clone, Copy)]
enum ProjectionKind {
    Todo,
    Agenda,
}

#[derive(Debug, Clone, Copy)]
struct ProjectionQuery<'a> {
    kind: ProjectionKind,
    valid_states: &'a [String],
    closed_states: &'a [String],
    clock: TaskClock,
}

pub(super) fn collect_todo_records(
    corpus: &Corpus,
    valid_states: &[String],
    clock: TaskClock,
) -> Vec<TaskRecord> {
    collect_records(
        corpus,
        ProjectionQuery {
            kind: ProjectionKind::Todo,
            valid_states,
            closed_states: &[],
            clock,
        },
    )
}

pub(super) fn collect_agenda_records(
    corpus: &Corpus,
    valid_states: &[String],
    closed_states: &[String],
    clock: TaskClock,
) -> Vec<TaskRecord> {
    collect_records(
        corpus,
        ProjectionQuery {
            kind: ProjectionKind::Agenda,
            valid_states,
            closed_states,
            clock,
        },
    )
}

fn collect_records(corpus: &Corpus, query: ProjectionQuery<'_>) -> Vec<TaskRecord> {
    let mut items = Vec::new();
    for result in corpus.results() {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path)
            .map(|date| TaskDateValue::new(date.format("%Y-%m-%d").to_string()));
        let has_agenda = parsed.filetags.iter().any(|tag| tag == "agenda");
        let primary_uuid = parsed.uuids.first().cloned().unwrap_or_default();
        let note_title = strip_org_links(&parsed.title.clone().unwrap_or_else(|| {
            path.file_stem()
                .map(|stem| stem.display().to_string())
                .unwrap_or_default()
        }));

        for heading in &parsed.headings {
            if !include_heading(query, heading, is_daily) {
                continue;
            }

            let item_is_overdue = is_overdue_on(heading.deadline.as_ref(), query.clock)
                || is_overdue_on(heading.scheduled.as_ref(), query.clock);
            let mut record = TaskRecord {
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
                todo_state: heading
                    .todo_state
                    .as_ref()
                    .map(|state| TaskState::new(state.as_str())),
                priority: heading.priority.map(task_priority),
                project: heading.project.clone().or_else(|| parsed.project.clone()),
                scheduled: heading.scheduled.clone(),
                scheduled_date: extract_date(heading.scheduled.as_ref()),
                deadline: heading.deadline.clone(),
                deadline_date: extract_date(heading.deadline.as_ref()),
                is_overdue: item_is_overdue,
                heading_tags: heading.tags.clone(),
            };
            if matches!(query.kind, ProjectionKind::Agenda)
                && record.is_daily_file
                && record.scheduled.is_none()
                && record.deadline.is_none()
            {
                record.is_overdue = record
                    .daily_file_date
                    .as_ref()
                    .and_then(TaskDateValue::parse_naive_date)
                    .is_some_and(|date| date < query.clock.today);
            }
            items.push(record);
        }
    }
    items
}

fn include_heading(query: ProjectionQuery<'_>, heading: &Heading, is_daily: bool) -> bool {
    match query.kind {
        ProjectionKind::Todo => heading
            .todo_state
            .as_ref()
            .is_some_and(|state| has_state(query.valid_states, state)),
        ProjectionKind::Agenda => include_agenda_heading(query, heading, is_daily),
    }
}

fn include_agenda_heading(query: ProjectionQuery<'_>, heading: &Heading, is_daily: bool) -> bool {
    let eligible = heading.scheduled.is_some()
        || heading.deadline.is_some()
        || (is_daily
            && heading
                .todo_state
                .as_ref()
                .is_some_and(|state| has_state(query.valid_states, state)));
    if !eligible {
        return false;
    }

    if !query.closed_states.is_empty()
        && let Some(todo_state) = &heading.todo_state
        && has_state(query.closed_states, todo_state)
    {
        return false;
    }

    true
}

fn has_state(states: &[String], value: &OrgTodoState) -> bool {
    states.iter().any(|state| state.eq_ignore_ascii_case(value))
}

fn extract_date(raw: Option<&String>) -> Option<TaskDateValue> {
    let parsed = parse_org_date(raw.as_ref()?)?;
    Some(TaskDateValue::new(
        parsed.base_date.format("%Y-%m-%d").to_string(),
    ))
}

fn is_overdue_on(raw: Option<&String>, clock: TaskClock) -> bool {
    let parsed = match raw.and_then(|raw| parse_org_date(raw)) {
        Some(parsed) => parsed,
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

fn task_priority(priority: OrgPriority) -> TaskPriority {
    match priority {
        OrgPriority::A => TaskPriority::A,
        OrgPriority::B => TaskPriority::B,
        OrgPriority::C => TaskPriority::C,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    #[test]
    fn projects_org_headings_into_task_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.org");
        std::fs::write(
            &path,
            ":PROPERTIES:\n:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\n:PROJECT: Work\n:END:\n#+title: [[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Linked Note]]\n#+filetags: :agenda:work:\n\n* TODO [#A] [[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][Call supplier]] :phone:\nSCHEDULED: <2026-05-23 Sat>\n",
        )
        .unwrap();
        let corpus = Corpus::scan(dir.path(), &[]).unwrap();
        let states = vec!["TODO".to_string()];

        let records = collect_todo_records(
            &corpus,
            &states,
            TaskClock {
                today: NaiveDate::from_ymd_opt(2026, 5, 24).unwrap(),
                now: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            },
        );

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.title, "Linked Note");
        assert_eq!(record.filetags, vec!["agenda", "work"]);
        assert_eq!(record.heading_title, "Call supplier");
        assert_eq!(record.priority, Some(TaskPriority::A));
        assert_eq!(record.project.as_deref(), Some("Work"));
        assert_eq!(
            record.scheduled_date.as_ref().map(TaskDateValue::as_str),
            Some("2026-05-23")
        );
        assert!(record.is_overdue);
        assert_eq!(record.heading_tags, vec!["phone"]);
    }
}
