use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};

use crate::clock::TaskClock;
use crate::id::TaskId;
use crate::model::{TaskDateValue, TaskPriority, TaskSourceKind, TaskState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDateArg {
    Set(TaskDateValue),
    Clear,
}

impl TaskDateArg {
    fn parse(name: &str, value: &str, today: NaiveDate, allow_clear: bool) -> Result<Self> {
        let value = value.trim();
        if allow_clear && is_clear_value(value) {
            return Ok(TaskDateArg::Clear);
        }
        parse_task_date_arg_on(name, value, today).map(TaskDateArg::Set)
    }

    pub fn as_value(&self) -> Option<&TaskDateValue> {
        match self {
            TaskDateArg::Set(value) => Some(value),
            TaskDateArg::Clear => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriorityArg {
    Set(TaskPriority),
    Clear,
}

impl TaskPriorityArg {
    fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if is_clear_value(value) {
            Ok(TaskPriorityArg::Clear)
        } else {
            TaskPriority::parse(value).map(TaskPriorityArg::Set)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDependencyArg {
    Set(TaskId),
    Clear,
}

impl TaskDependencyArg {
    fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            Ok(TaskDependencyArg::Clear)
        } else {
            value.parse().map(TaskDependencyArg::Set)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskModifierMode {
    Add,
    Mod,
}

impl TaskModifierMode {
    fn due_name(self) -> &'static str {
        match self {
            TaskModifierMode::Add => "due",
            TaskModifierMode::Mod => "schedule",
        }
    }

    fn allows_date_clear(self) -> bool {
        matches!(self, TaskModifierMode::Mod)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskModifierSpec {
    pub source: Option<TaskSourceKind>,
    pub project: Option<String>,
    pub title: Option<String>,
    pub due: Option<TaskDateArg>,
    pub deadline: Option<TaskDateArg>,
    pub labels: Option<Vec<String>>,
    pub priority: Option<TaskPriorityArg>,
    pub state: Option<TaskState>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub dependency: Option<TaskDependencyArg>,
    pub text: Option<String>,
}

impl TaskModifierSpec {
    pub fn parse(tokens: &[String]) -> Result<Self> {
        Self::parse_on(tokens, TaskClock::now().today)
    }

    pub fn parse_on(tokens: &[String], today: NaiveDate) -> Result<Self> {
        Self::parse_with_mode(tokens, today, TaskModifierMode::Add)
    }

    pub fn parse_mod(tokens: &[String]) -> Result<Self> {
        Self::parse_mod_on(tokens, TaskClock::now().today)
    }

    pub fn parse_mod_on(tokens: &[String], today: NaiveDate) -> Result<Self> {
        Self::parse_with_mode(tokens, today, TaskModifierMode::Mod)
    }

    fn parse_with_mode(
        tokens: &[String],
        today: NaiveDate,
        mode: TaskModifierMode,
    ) -> Result<Self> {
        let mut spec = TaskModifierSpec::default();
        let mut text = Vec::new();

        for token in tokens {
            if apply_modifier(&mut spec, token, today, mode)? {
                continue;
            }
            if mode == TaskModifierMode::Mod {
                bail!(
                    "Unknown task modifier '{}'. Use title:<text> to change a task title.",
                    token
                );
            }
            text.push(token.clone());
        }

        if !text.is_empty() {
            set_once(&mut spec.text, "text", text.join(" "))?;
        }

        Ok(spec)
    }

    pub fn source_or_default(&self) -> TaskSourceKind {
        self.source.unwrap_or(TaskSourceKind::Pkms)
    }

    pub fn labels(&self) -> &[String] {
        match self.labels.as_deref() {
            Some(labels) => labels,
            None => &[],
        }
    }
}

fn apply_modifier(
    spec: &mut TaskModifierSpec,
    token: &str,
    today: NaiveDate,
    mode: TaskModifierMode,
) -> Result<bool> {
    let Some((key, value)) = token.split_once(':') else {
        return Ok(false);
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    if key == "status" {
        bail!("task modifier status: was renamed to state:");
    }

    match resolve_modifier_key(&key)? {
        Some(ModifierKey::Source) => {
            set_once(&mut spec.source, "source", value.parse()?)?;
        }
        Some(ModifierKey::Title) => {
            set_once(&mut spec.title, "title", value.to_string())?;
        }
        Some(ModifierKey::Labels) => {
            spec.labels
                .get_or_insert_with(Vec::new)
                .extend(split_list(value));
        }
        Some(ModifierKey::Due) => {
            set_once(
                &mut spec.due,
                "schedule",
                TaskDateArg::parse(mode.due_name(), value, today, mode.allows_date_clear())?,
            )?;
        }
        Some(ModifierKey::Deadline) => {
            set_once(
                &mut spec.deadline,
                "deadline",
                TaskDateArg::parse("deadline", value, today, mode.allows_date_clear())?,
            )?;
        }
        Some(ModifierKey::Project) => {
            set_once(&mut spec.project, "project", value.to_string())?;
        }
        Some(ModifierKey::Priority) => {
            set_once(
                &mut spec.priority,
                "priority",
                TaskPriorityArg::parse(value)?,
            )?;
        }
        Some(ModifierKey::State) => {
            set_once(&mut spec.state, "state", TaskState::new(value))?;
        }
        Some(ModifierKey::Description) => {
            set_once(&mut spec.description, "description", value.to_string())?;
        }
        Some(ModifierKey::Note) => {
            set_once(&mut spec.note, "note", value.to_string())?;
        }
        Some(ModifierKey::Dependency) => {
            set_once(
                &mut spec.dependency,
                "dep",
                TaskDependencyArg::parse(value)?,
            )?;
        }
        None => return Ok(false),
    }
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierKey {
    Source,
    Title,
    Labels,
    Due,
    Deadline,
    Project,
    Priority,
    State,
    Description,
    Note,
    Dependency,
}

impl ModifierKey {
    fn name(self) -> &'static str {
        match self {
            ModifierKey::Source => "source",
            ModifierKey::Title => "title",
            ModifierKey::Labels => "tag",
            ModifierKey::Due => "schedule",
            ModifierKey::Deadline => "deadline",
            ModifierKey::Project => "project",
            ModifierKey::Priority => "priority",
            ModifierKey::State => "state",
            ModifierKey::Description => "description",
            ModifierKey::Note => "note",
            ModifierKey::Dependency => "dep",
        }
    }
}

const MODIFIER_KEYS: &[(&str, ModifierKey)] = &[
    ("source", ModifierKey::Source),
    ("src", ModifierKey::Source),
    ("title", ModifierKey::Title),
    ("tag", ModifierKey::Labels),
    ("tags", ModifierKey::Labels),
    ("label", ModifierKey::Labels),
    ("labels", ModifierKey::Labels),
    ("due", ModifierKey::Due),
    ("schedule", ModifierKey::Due),
    ("scheduled", ModifierKey::Due),
    ("sched", ModifierKey::Due),
    ("sch", ModifierKey::Due),
    ("deadline", ModifierKey::Deadline),
    ("dead", ModifierKey::Deadline),
    ("dl", ModifierKey::Deadline),
    ("project", ModifierKey::Project),
    ("proj", ModifierKey::Project),
    ("priority", ModifierKey::Priority),
    ("prio", ModifierKey::Priority),
    ("pri", ModifierKey::Priority),
    ("state", ModifierKey::State),
    ("description", ModifierKey::Description),
    ("desc", ModifierKey::Description),
    ("body", ModifierKey::Description),
    ("note", ModifierKey::Note),
    ("dep", ModifierKey::Dependency),
    ("depend", ModifierKey::Dependency),
];

fn resolve_modifier_key(key: &str) -> Result<Option<ModifierKey>> {
    if key.is_empty() {
        return Ok(None);
    }

    for (candidate, modifier) in MODIFIER_KEYS {
        if *candidate == key {
            return Ok(Some(*modifier));
        }
    }

    let mut matches = Vec::new();
    for (candidate, modifier) in MODIFIER_KEYS {
        if candidate.starts_with(key) && !matches.contains(modifier) {
            matches.push(*modifier);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [modifier] => Ok(Some(*modifier)),
        _ => {
            let names = matches
                .iter()
                .map(|modifier| modifier.name())
                .collect::<Vec<_>>()
                .join(", ");
            bail!("Ambiguous task modifier '{key}'. Could match: {names}. Use a longer modifier.")
        }
    }
}

fn split_list(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn set_once<T>(target: &mut Option<T>, name: &str, value: T) -> Result<()> {
    if target.is_some() {
        bail!("task modifier {name} was provided more than once");
    }
    *target = Some(value);
    Ok(())
}

pub fn is_clear_value(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value.eq_ignore_ascii_case("none")
}

pub fn pkms_priority(value: &str) -> Result<TaskPriority> {
    TaskPriority::parse(value)
}

pub fn parse_task_date_arg(name: &str, value: &str) -> Result<TaskDateValue> {
    parse_task_date_arg_on(name, value, TaskClock::now().today)
}

pub fn parse_task_date_arg_on(name: &str, value: &str, today: NaiveDate) -> Result<TaskDateValue> {
    if let Some(date) = parse_word_date(name, value, today)? {
        return Ok(TaskDateValue::new(date.format("%Y-%m-%d").to_string()));
    }
    if let Some(date) = parse_iso_date(value) {
        return Ok(TaskDateValue::new(date.format("%Y-%m-%d").to_string()));
    }
    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M") {
        return Ok(TaskDateValue::new(
            datetime.format("%Y-%m-%d %H:%M").to_string(),
        ));
    }
    Err(anyhow::anyhow!(
        "Invalid {name} date '{value}'. Use an unambiguous prefix of today, tomorrow, or a weekday; YYYY-MM-DD; or YYYY-MM-DD HH:MM."
    ))
}

#[derive(Copy, Clone)]
enum DateWordKind {
    Today,
    Tomorrow,
    Weekday(Weekday),
}

struct DateWord {
    word: &'static str,
    min_prefix_len: usize,
    kind: DateWordKind,
}

const DATE_WORDS: &[DateWord] = &[
    DateWord {
        word: "today",
        min_prefix_len: 1,
        kind: DateWordKind::Today,
    },
    DateWord {
        word: "tomorrow",
        min_prefix_len: 3,
        kind: DateWordKind::Tomorrow,
    },
    DateWord {
        word: "monday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Mon),
    },
    DateWord {
        word: "tuesday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Tue),
    },
    DateWord {
        word: "wednesday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Wed),
    },
    DateWord {
        word: "thursday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Thu),
    },
    DateWord {
        word: "friday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Fri),
    },
    DateWord {
        word: "saturday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Sat),
    },
    DateWord {
        word: "sunday",
        min_prefix_len: 1,
        kind: DateWordKind::Weekday(Weekday::Sun),
    },
];

fn parse_word_date(name: &str, value: &str, today: NaiveDate) -> Result<Option<NaiveDate>> {
    let normalized = value.to_ascii_lowercase();
    let matches = DATE_WORDS
        .iter()
        .filter(|candidate| {
            normalized.len() >= candidate.min_prefix_len
                && candidate.word.starts_with(normalized.as_str())
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(match candidate.kind {
            DateWordKind::Today => today,
            DateWordKind::Tomorrow => today + chrono::Duration::days(1),
            DateWordKind::Weekday(weekday) => upcoming_weekday(today, weekday),
        })),
        _ => {
            let words = matches
                .iter()
                .map(|candidate| candidate.word)
                .collect::<Vec<_>>()
                .join(", ");
            bail!("Ambiguous {name} date '{value}'. Could match: {words}. Use a longer date word.")
        }
    }
}

fn parse_iso_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn upcoming_weekday(today: NaiveDate, weekday: Weekday) -> NaiveDate {
    let today_index = today.weekday().num_days_from_monday() as i64;
    let target_index = weekday.num_days_from_monday() as i64;
    let mut days_until = (target_index - today_index).rem_euclid(7);
    if days_until == 0 {
        days_until = 7;
    }
    today + chrono::Duration::days(days_until)
}

pub fn org_date(date: &TaskDateValue) -> Result<String> {
    if let Ok(datetime) = NaiveDateTime::parse_from_str(date.as_str(), "%Y-%m-%d %H:%M") {
        return Ok(format!("<{}>", datetime.format("%Y-%m-%d %a %H:%M")));
    }
    let date = NaiveDate::parse_from_str(date.as_str(), "%Y-%m-%d")?;
    Ok(format!("<{}>", date.format("%Y-%m-%d %a")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn date_value(value: Option<&TaskDateArg>) -> Option<&str> {
        value
            .and_then(TaskDateArg::as_value)
            .map(TaskDateValue::as_str)
    }

    #[test]
    fn parses_structured_task_add_tokens() {
        let spec = TaskModifierSpec::parse(&tokens(&[
            "source:todoist",
            "title:Call Alice",
            "tag:phone,urgent",
            "sch:2026-05-27",
            "dead:2026-05-28",
            "prio:A",
            "project:Inbox",
            "desc:Follow up",
            "state:waiting",
            "depend:2",
        ]))
        .unwrap();

        assert_eq!(spec.source.map(|source| source.as_str()), Some("todoist"));
        assert_eq!(spec.title.as_deref(), Some("Call Alice"));
        assert_eq!(
            spec.labels,
            Some(vec!["phone".to_string(), "urgent".to_string()])
        );
        assert_eq!(date_value(spec.due.as_ref()), Some("2026-05-27"));
        assert_eq!(date_value(spec.deadline.as_ref()), Some("2026-05-28"));
        assert_eq!(spec.priority, Some(TaskPriorityArg::Set(TaskPriority::A)));
        assert_eq!(spec.project.as_deref(), Some("Inbox"));
        assert_eq!(spec.description.as_deref(), Some("Follow up"));
        assert_eq!(spec.state.as_deref(), Some("waiting"));
        assert_eq!(
            spec.dependency,
            Some(TaskDependencyArg::Set(crate::id::TaskId::Pkms(2)))
        );
    }

    #[test]
    fn parses_add_dates_into_values_against_explicit_today() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let spec =
            TaskModifierSpec::parse_on(&tokens(&["sch:tom", "dead:2026-05-29"]), today).unwrap();

        assert_eq!(
            spec.due,
            Some(TaskDateArg::Set(TaskDateValue::from("2026-05-28")))
        );
        assert_eq!(
            spec.deadline,
            Some(TaskDateArg::Set(TaskDateValue::from("2026-05-29")))
        );
    }

    #[test]
    fn parses_mod_date_clears_as_typed_values() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let spec = TaskModifierSpec::parse_mod_on(&tokens(&["sch:none", "dead:"]), today).unwrap();

        assert_eq!(spec.due, Some(TaskDateArg::Clear));
        assert_eq!(spec.deadline, Some(TaskDateArg::Clear));
    }

    #[test]
    fn parses_dependency_modifiers_as_typed_values() {
        let spec = TaskModifierSpec::parse_mod(&tokens(&["dep:p2"])).unwrap();
        assert_eq!(
            spec.dependency,
            Some(TaskDependencyArg::Set(crate::id::TaskId::Pkms(2)))
        );

        let spec = TaskModifierSpec::parse_mod(&tokens(&["dep:"])).unwrap();
        assert_eq!(spec.dependency, Some(TaskDependencyArg::Clear));
    }

    #[test]
    fn parses_unambiguous_modifier_key_prefixes() {
        let spec = TaskModifierSpec::parse(&tokens(&[
            "sou:todoist",
            "tit:Call Alice",
            "ta:phone,urgent",
            "sche:2026-05-27",
            "deadl:2026-05-28",
            "pro:Inbox",
            "prior:A",
            "descr:Follow up",
            "not:Project Note",
        ]))
        .unwrap();

        assert_eq!(spec.source.map(|source| source.as_str()), Some("todoist"));
        assert_eq!(spec.title.as_deref(), Some("Call Alice"));
        assert_eq!(
            spec.labels,
            Some(vec!["phone".to_string(), "urgent".to_string()])
        );
        assert_eq!(date_value(spec.due.as_ref()), Some("2026-05-27"));
        assert_eq!(date_value(spec.deadline.as_ref()), Some("2026-05-28"));
        assert_eq!(spec.project.as_deref(), Some("Inbox"));
        assert_eq!(spec.priority, Some(TaskPriorityArg::Set(TaskPriority::A)));
        assert_eq!(spec.description.as_deref(), Some("Follow up"));
        assert_eq!(spec.note.as_deref(), Some("Project Note"));
    }

    #[test]
    fn rejects_ambiguous_modifier_key_prefixes() {
        let err = TaskModifierSpec::parse_mod(&tokens(&["pr:Inbox"])).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Ambiguous task modifier 'pr'"));
        assert!(message.contains("project"));
        assert!(message.contains("priority"));
    }

    #[test]
    fn keeps_plain_words_as_task_text() {
        let spec = TaskModifierSpec::parse(&tokens(&["Call", "Alice", "tag:phone"])).unwrap();
        assert_eq!(spec.source_or_default(), TaskSourceKind::Pkms);
        assert_eq!(spec.text.as_deref(), Some("Call Alice"));
        assert_eq!(spec.labels, Some(vec!["phone".to_string()]));
    }

    #[test]
    fn rejects_plain_words_for_task_mod() {
        let err = TaskModifierSpec::parse_mod(&tokens(&["Call", "Alice"])).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Unknown task modifier 'Call'"));
        assert!(message.contains("title:<text>"));
    }

    #[test]
    fn rejects_unknown_key_for_task_mod() {
        let err = TaskModifierSpec::parse_mod(&tokens(&["unknown:value"])).unwrap_err();
        assert!(
            err.to_string()
                .contains("Unknown task modifier 'unknown:value'")
        );
    }

    #[test]
    fn rejects_legacy_status_modifier_for_task_add() {
        let err = TaskModifierSpec::parse(&tokens(&["status:waiting"])).unwrap_err();
        assert!(err.to_string().contains("state:"));
    }

    #[test]
    fn rejects_legacy_status_modifier_for_task_mod() {
        let err = TaskModifierSpec::parse_mod(&tokens(&["status:waiting"])).unwrap_err();
        assert!(err.to_string().contains("state:"));
    }

    #[test]
    fn rejects_duplicate_single_value_options() {
        let err = TaskModifierSpec::parse(&tokens(&["title:One", "title:Two"])).unwrap_err();
        assert!(
            err.to_string()
                .contains("task modifier title was provided more than once")
        );
    }

    #[test]
    fn formats_org_dates() {
        assert_eq!(
            org_date(&TaskDateValue::from("2026-05-27")).unwrap(),
            "<2026-05-27 Wed>"
        );
        assert_eq!(
            org_date(&TaskDateValue::from("2026-05-27 09:30")).unwrap(),
            "<2026-05-27 Wed 09:30>"
        );
    }

    #[test]
    fn parses_explicit_add_dates() {
        assert_eq!(
            parse_task_date_arg("due", "2026-05-27").unwrap(),
            "2026-05-27"
        );
        assert_eq!(
            parse_task_date_arg("due", "2026-05-27 09:30").unwrap(),
            "2026-05-27 09:30"
        );
    }

    #[test]
    fn parses_relative_add_dates_against_explicit_today() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        assert_eq!(
            parse_task_date_arg_on("due", "today", today).unwrap(),
            "2026-05-27"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "tom", today).unwrap(),
            "2026-05-28"
        );
    }

    #[test]
    fn parses_weekday_add_dates_as_upcoming_days() {
        let friday = NaiveDate::from_ymd_opt(2026, 5, 29).unwrap();
        assert_eq!(
            parse_task_date_arg_on("due", "mon", friday).unwrap(),
            "2026-06-01"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "monday", friday).unwrap(),
            "2026-06-01"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "fri", friday).unwrap(),
            "2026-06-05"
        );
    }

    #[test]
    fn parses_unambiguous_word_prefix_dates_case_insensitively() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).unwrap();
        assert_eq!(
            parse_task_date_arg_on("due", "Tod", today).unwrap(),
            "2026-05-29"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "toda", today).unwrap(),
            "2026-05-29"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "to", today).unwrap(),
            "2026-05-29"
        );
        assert_eq!(
            parse_task_date_arg_on("due", "thu", today).unwrap(),
            "2026-06-04"
        );
    }

    #[test]
    fn rejects_ambiguous_word_prefix_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 29).unwrap();
        let err = parse_task_date_arg_on("due", "t", today).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Ambiguous due date 't'"));
        assert!(message.contains("today"));
        assert!(message.contains("tuesday"));
        assert!(message.contains("thursday"));
    }
}
