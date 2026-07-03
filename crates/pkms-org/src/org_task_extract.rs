use crate::corpus::Corpus;
use crate::domain::NoteId;
use crate::org_date::parse_org_date;
use crate::parser::{Heading, OrgPriority, OrgTodoState, find_daily_file_date, strip_org_links};
use chrono::{NaiveDate, NaiveTime};

#[derive(Debug, Clone)]
pub struct OrgTaskRecord {
    pub uuid: NoteId,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub has_agenda_tag: bool,
    pub is_daily_file: bool,
    pub daily_file_date: Option<String>,
    pub heading_title: String,
    pub heading_level: usize,
    pub line_number: usize,
    pub todo_state: Option<OrgTodoState>,
    pub priority: Option<OrgPriority>,
    pub project: Option<String>,
    pub scheduled: Option<String>,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub deadline_date: Option<String>,
    pub is_overdue: bool,
    pub heading_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct OrgTaskClock {
    pub today: NaiveDate,
    pub now: NaiveTime,
}

#[derive(Debug, Clone, Copy)]
enum OrgTaskRecordKind {
    Todo,
    Agenda,
}

#[derive(Debug, Clone, Copy)]
pub struct OrgTaskRecordQuery<'a> {
    kind: OrgTaskRecordKind,
    valid_states: &'a [String],
    closed_states: &'a [String],
    clock: OrgTaskClock,
}

impl<'a> OrgTaskRecordQuery<'a> {
    pub fn todo(valid_states: &'a [String], clock: OrgTaskClock) -> Self {
        Self {
            kind: OrgTaskRecordKind::Todo,
            valid_states,
            closed_states: &[],
            clock,
        }
    }

    pub fn agenda(
        valid_states: &'a [String],
        closed_states: &'a [String],
        clock: OrgTaskClock,
    ) -> Self {
        Self {
            kind: OrgTaskRecordKind::Agenda,
            valid_states,
            closed_states,
            clock,
        }
    }
}

pub fn collect_org_task_records(
    corpus: &Corpus,
    query: OrgTaskRecordQuery<'_>,
) -> Vec<OrgTaskRecord> {
    let mut items = Vec::new();
    for result in corpus.results() {
        if result.parse_error.is_some() {
            continue;
        }

        let parsed = &result.parsed;
        let path = &result.path;
        let is_daily = find_daily_file_date(path).is_some();
        let daily_date = find_daily_file_date(path).map(|date| date.format("%Y-%m-%d").to_string());
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
            let mut record = OrgTaskRecord {
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
            };
            if matches!(query.kind, OrgTaskRecordKind::Agenda)
                && record.is_daily_file
                && record.scheduled.is_none()
                && record.deadline.is_none()
            {
                record.is_overdue = record
                    .daily_file_date
                    .as_deref()
                    .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
                    .is_some_and(|date| date < query.clock.today);
            }
            items.push(record);
        }
    }
    items
}

fn include_heading(query: OrgTaskRecordQuery<'_>, heading: &Heading, is_daily: bool) -> bool {
    match query.kind {
        OrgTaskRecordKind::Todo => heading
            .todo_state
            .as_ref()
            .is_some_and(|state| has_state(query.valid_states, state)),
        OrgTaskRecordKind::Agenda => include_agenda_heading(query, heading, is_daily),
    }
}

fn include_agenda_heading(
    query: OrgTaskRecordQuery<'_>,
    heading: &Heading,
    is_daily: bool,
) -> bool {
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

fn extract_date(raw: Option<&String>) -> Option<String> {
    let parsed = parse_org_date(raw.as_ref()?)?;
    Some(parsed.base_date.format("%Y-%m-%d").to_string())
}

fn is_overdue_on(raw: Option<&String>, clock: OrgTaskClock) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_org_task_records_from_todo_headings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.org");
        std::fs::write(
            &path,
            ":PROPERTIES:\n:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\n:PROJECT: Work\n:END:\n#+title: [[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][Linked Note]]\n#+filetags: :agenda:work:\n\n* TODO [#A] [[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][Call supplier]] :phone:\nSCHEDULED: <2026-05-23 Sat>\n",
        )
        .unwrap();
        let corpus = Corpus::scan(dir.path(), &[]).unwrap();
        let states = vec!["TODO".to_string()];

        let records = collect_org_task_records(
            &corpus,
            OrgTaskRecordQuery::todo(
                &states,
                OrgTaskClock {
                    today: NaiveDate::from_ymd_opt(2026, 5, 24).unwrap(),
                    now: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                },
            ),
        );

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.title, "Linked Note");
        assert_eq!(record.filetags, vec!["agenda", "work"]);
        assert_eq!(record.heading_title, "Call supplier");
        assert_eq!(record.priority, Some(OrgPriority::A));
        assert_eq!(record.project.as_deref(), Some("Work"));
        assert_eq!(record.scheduled_date.as_deref(), Some("2026-05-23"));
        assert!(record.is_overdue);
        assert_eq!(record.heading_tags, vec!["phone"]);
    }
}
