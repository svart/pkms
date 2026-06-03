use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};

use crate::tasks::clock::TaskClock;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskModifierSpec {
    pub source: Option<String>,
    pub project: Option<String>,
    pub title: Option<String>,
    pub due: Option<String>,
    pub deadline: Option<String>,
    pub labels: Option<Vec<String>>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub dependency: Option<String>,
    pub text: Option<String>,
}

impl TaskModifierSpec {
    pub fn parse(tokens: &[String]) -> Result<Self> {
        let mut spec = TaskModifierSpec::default();
        let mut text = Vec::new();

        for token in tokens {
            if apply_modifier(&mut spec, token)? {
                continue;
            }
            text.push(token.clone());
        }

        if !text.is_empty() {
            set_once(&mut spec.text, "text", text.join(" "))?;
        }

        Ok(spec)
    }

    pub fn parse_mod(tokens: &[String]) -> Result<Self> {
        let mut spec = TaskModifierSpec::default();

        for token in tokens {
            if apply_modifier(&mut spec, token)? {
                continue;
            }
            bail!(
                "Unknown task modifier '{}'. Use title:<text> to change a task title.",
                token
            );
        }

        Ok(spec)
    }

    pub fn source_or_default(&self) -> &str {
        self.source.as_deref().unwrap_or("pkms")
    }

    pub fn labels(&self) -> &[String] {
        match self.labels.as_deref() {
            Some(labels) => labels,
            None => &[],
        }
    }
}

fn apply_modifier(spec: &mut TaskModifierSpec, token: &str) -> Result<bool> {
    let Some((key, value)) = token.split_once(':') else {
        return Ok(false);
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    match key.as_str() {
        "source" | "src" => {
            spec.source = Some(value.to_string());
        }
        "title" => {
            set_once(&mut spec.title, "title", value.to_string())?;
        }
        "tag" | "tags" | "label" | "labels" => {
            spec.labels
                .get_or_insert_with(Vec::new)
                .extend(split_list(value));
        }
        "due" | "schedule" | "scheduled" | "sched" | "sch" => {
            set_once(&mut spec.due, "schedule", value.to_string())?;
        }
        "deadline" | "dead" | "dl" => {
            set_once(&mut spec.deadline, "deadline", value.to_string())?;
        }
        "project" | "proj" => {
            set_once(&mut spec.project, "project", value.to_string())?;
        }
        "priority" | "prio" | "pri" => {
            set_once(&mut spec.priority, "priority", value.to_string())?;
        }
        "status" => {
            set_once(&mut spec.status, "status", value.to_string())?;
        }
        "description" | "desc" | "body" => {
            set_once(&mut spec.description, "description", value.to_string())?;
        }
        "note" => {
            set_once(&mut spec.note, "note", value.to_string())?;
        }
        "dep" | "depend" => {
            set_once(&mut spec.dependency, "dep", value.to_string())?;
        }
        _ => return Ok(false),
    }
    Ok(true)
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

pub fn pkms_priority(value: &str) -> Result<char> {
    match value.to_ascii_uppercase().as_str() {
        "A" | "B" | "C" => Ok(value.to_ascii_uppercase().chars().next().unwrap()),
        _ => bail!("Invalid priority '{value}'. Use A, B, or C."),
    }
}

pub fn parse_task_date_arg(name: &str, value: &str) -> Result<String> {
    parse_task_date_arg_on(name, value, TaskClock::now().today)
}

pub fn parse_task_date_arg_on(name: &str, value: &str, today: NaiveDate) -> Result<String> {
    if let Some(date) = parse_word_date(name, value, today)? {
        return Ok(date.format("%Y-%m-%d").to_string());
    }
    if let Some(date) = crate::input::parse_date(Some(value)) {
        return Ok(date.format("%Y-%m-%d").to_string());
    }
    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M") {
        return Ok(datetime.format("%Y-%m-%d %H:%M").to_string());
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

fn upcoming_weekday(today: NaiveDate, weekday: Weekday) -> NaiveDate {
    let today_index = today.weekday().num_days_from_monday() as i64;
    let target_index = weekday.num_days_from_monday() as i64;
    let mut days_until = (target_index - today_index).rem_euclid(7);
    if days_until == 0 {
        days_until = 7;
    }
    today + chrono::Duration::days(days_until)
}

pub fn validate_pkms_task_date_arg(name: &str, value: Option<&str>) -> Result<Option<String>> {
    validate_pkms_task_date_arg_on(name, value, TaskClock::now().today)
}

pub fn validate_pkms_task_date_arg_on(
    name: &str,
    value: Option<&str>,
    today: NaiveDate,
) -> Result<Option<String>> {
    value
        .map(|value| parse_task_date_arg_on(name, value, today))
        .transpose()
}

pub fn org_date(date: &str) -> Result<String> {
    if let Ok(datetime) = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M") {
        return Ok(format!("<{}>", datetime.format("%Y-%m-%d %a %H:%M")));
    }
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    Ok(format!("<{}>", date.format("%Y-%m-%d %a")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
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
            "status:waiting",
            "depend:2",
        ]))
        .unwrap();

        assert_eq!(spec.source.as_deref(), Some("todoist"));
        assert_eq!(spec.title.as_deref(), Some("Call Alice"));
        assert_eq!(
            spec.labels,
            Some(vec!["phone".to_string(), "urgent".to_string()])
        );
        assert_eq!(spec.due.as_deref(), Some("2026-05-27"));
        assert_eq!(spec.deadline.as_deref(), Some("2026-05-28"));
        assert_eq!(spec.priority.as_deref(), Some("A"));
        assert_eq!(spec.project.as_deref(), Some("Inbox"));
        assert_eq!(spec.description.as_deref(), Some("Follow up"));
        assert_eq!(spec.status.as_deref(), Some("waiting"));
        assert_eq!(spec.dependency.as_deref(), Some("2"));
    }

    #[test]
    fn keeps_plain_words_as_task_text() {
        let spec = TaskModifierSpec::parse(&tokens(&["Call", "Alice", "tag:phone"])).unwrap();
        assert_eq!(spec.source_or_default(), "pkms");
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
    fn rejects_duplicate_single_value_options() {
        let err = TaskModifierSpec::parse(&tokens(&["title:One", "title:Two"])).unwrap_err();
        assert!(
            err.to_string()
                .contains("task modifier title was provided more than once")
        );
    }

    #[test]
    fn formats_org_dates() {
        assert_eq!(org_date("2026-05-27").unwrap(), "<2026-05-27 Wed>");
        assert_eq!(
            org_date("2026-05-27 09:30").unwrap(),
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
