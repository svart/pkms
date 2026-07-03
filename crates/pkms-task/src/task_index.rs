use crate::clock::TaskClock;
use crate::filter::{TextFilter, matches_tag_filters, matches_text_filters, matches_type_filters};
use crate::model::{TaskDateValue, TaskPriority, TaskState};
use pkms_org::Graph;
use pkms_org::corpus::Corpus;
use pkms_org::domain::NoteId;
use pkms_org::graph::tasks::TaskStateConfig;
use pkms_org::org_task_extract::{
    OrgTaskClock, OrgTaskRecord, OrgTaskRecordQuery, collect_org_task_records,
};
use pkms_org::parser::OrgPriority;

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

    pub fn has_effective_date(&self, date: &str) -> bool {
        self.scheduled_date.as_ref().map(TaskDateValue::as_str) == Some(date)
            || self.deadline_date.as_ref().map(TaskDateValue::as_str) == Some(date)
            || self.implicit_daily_file_date() == Some(date)
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

    pub fn with_filters(mut self, filters: RecordFilters<'a>) -> Self {
        self.filters = filters;
        self
    }
}

pub fn collect_todo_records(corpus: &Corpus, query: TaskRecordQuery<'_>) -> Vec<TaskRecord> {
    let TaskRecordQuery {
        valid_states,
        filters,
        clock,
        ..
    } = query;

    collect_org_task_records(
        corpus,
        OrgTaskRecordQuery::todo(valid_states, org_task_clock(clock)),
    )
    .into_iter()
    .map(task_record_from_org)
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

    collect_org_task_records(
        corpus,
        OrgTaskRecordQuery::agenda(valid_states, closed_states, org_task_clock(clock)),
    )
    .into_iter()
    .map(task_record_from_org)
    .filter(|record| apply_common_filters(record, filters))
    .collect()
}

pub fn assign_canonical_ids(
    task_states: &TaskStateConfig,
    graph: &Graph,
    records: &mut [TaskRecord],
) {
    let global_ids: std::collections::HashMap<(String, usize), usize> = graph
        .all_task_entries(task_states)
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

fn org_task_clock(clock: TaskClock) -> OrgTaskClock {
    OrgTaskClock {
        today: clock.today,
        now: clock.now,
    }
}

fn task_record_from_org(record: OrgTaskRecord) -> TaskRecord {
    TaskRecord {
        id: 0,
        uuid: record.uuid,
        title: record.title,
        path: record.path,
        filetags: record.filetags,
        has_agenda_tag: record.has_agenda_tag,
        is_daily_file: record.is_daily_file,
        daily_file_date: record.daily_file_date.map(TaskDateValue::new),
        heading_title: record.heading_title,
        heading_level: record.heading_level,
        line_number: record.line_number,
        todo_state: record
            .todo_state
            .map(|state| TaskState::new(state.as_str())),
        priority: record.priority.map(task_priority),
        project: record.project,
        scheduled: record.scheduled,
        scheduled_date: record.scheduled_date.map(TaskDateValue::new),
        deadline: record.deadline,
        deadline_date: record.deadline_date.map(TaskDateValue::new),
        is_overdue: record.is_overdue,
        heading_tags: record.heading_tags,
    }
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

fn task_priority(priority: OrgPriority) -> TaskPriority {
    match priority {
        OrgPriority::A => TaskPriority::A,
        OrgPriority::B => TaskPriority::B,
        OrgPriority::C => TaskPriority::C,
    }
}
