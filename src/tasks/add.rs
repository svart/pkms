use anyhow::{Result, bail};
use chrono::{Local, NaiveDate, NaiveDateTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskAddSpec {
    pub source: String,
    pub project: Option<String>,
    pub title: Option<String>,
    pub due: Option<String>,
    pub deadline: Option<String>,
    pub labels: Vec<String>,
    pub priority: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub text: Option<String>,
}

impl TaskAddSpec {
    pub fn parse(tokens: &[String]) -> Result<Self> {
        let mut spec = TaskAddSpec {
            source: "pkms".to_string(),
            project: None,
            title: None,
            due: None,
            deadline: None,
            labels: Vec::new(),
            priority: None,
            description: None,
            note: None,
            text: None,
        };
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
}

fn apply_modifier(spec: &mut TaskAddSpec, token: &str) -> Result<bool> {
    let Some((key, value)) = token.split_once(':') else {
        return Ok(false);
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    match key.as_str() {
        "source" | "src" => spec.source = value.to_string(),
        "title" => set_once(&mut spec.title, "title", value.to_string())?,
        "tag" | "tags" | "label" | "labels" => spec.labels.extend(split_list(value)),
        "due" | "schedule" | "scheduled" | "sched" | "sch" => {
            set_once(&mut spec.due, "schedule", value.to_string())?;
        }
        "deadline" | "dead" | "dl" => {
            set_once(&mut spec.deadline, "deadline", value.to_string())?;
        }
        "project" | "proj" => set_once(&mut spec.project, "project", value.to_string())?,
        "priority" | "prio" | "pri" => {
            set_once(&mut spec.priority, "priority", value.to_string())?;
        }
        "description" | "desc" | "body" => {
            set_once(&mut spec.description, "description", value.to_string())?;
        }
        "note" => set_once(&mut spec.note, "note", value.to_string())?,
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
        bail!("task add {name} was provided more than once");
    }
    *target = Some(value);
    Ok(())
}

pub fn pkms_priority(value: &str) -> Result<char> {
    match value.to_ascii_uppercase().as_str() {
        "A" | "B" | "C" => Ok(value.to_ascii_uppercase().chars().next().unwrap()),
        _ => bail!("Invalid priority '{value}'. Use A, B, or C."),
    }
}

pub fn parse_add_date_arg(name: &str, value: &str) -> Result<String> {
    parse_add_date_arg_on(name, value, Local::now().date_naive())
}

pub fn parse_add_date_arg_on(name: &str, value: &str, today: NaiveDate) -> Result<String> {
    if value.eq_ignore_ascii_case("today") || value.eq_ignore_ascii_case("tod") {
        return Ok(today.format("%Y-%m-%d").to_string());
    }
    if value.eq_ignore_ascii_case("tomorrow") || value.eq_ignore_ascii_case("tom") {
        return Ok((today + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string());
    }
    if let Some(date) = crate::input::parse_date(Some(value)) {
        return Ok(date.format("%Y-%m-%d").to_string());
    }
    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M") {
        return Ok(datetime.format("%Y-%m-%d %H:%M").to_string());
    }
    Err(anyhow::anyhow!(
        "Invalid {name} date '{value}'. Use today, tomorrow, tod, tom, YYYY-MM-DD, or YYYY-MM-DD HH:MM."
    ))
}

pub fn validate_pkms_date_arg(name: &str, value: Option<&str>) -> Result<Option<String>> {
    validate_pkms_date_arg_on(name, value, Local::now().date_naive())
}

pub fn validate_pkms_date_arg_on(
    name: &str,
    value: Option<&str>,
    today: NaiveDate,
) -> Result<Option<String>> {
    value
        .map(|value| parse_add_date_arg_on(name, value, today))
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
        let spec = TaskAddSpec::parse(&tokens(&[
            "source:todoist",
            "title:Call Alice",
            "tag:phone,urgent",
            "sch:2026-05-27",
            "dead:2026-05-28",
            "prio:A",
            "project:Inbox",
            "desc:Follow up",
        ]))
        .unwrap();

        assert_eq!(spec.source, "todoist");
        assert_eq!(spec.title.as_deref(), Some("Call Alice"));
        assert_eq!(spec.labels, vec!["phone", "urgent"]);
        assert_eq!(spec.due.as_deref(), Some("2026-05-27"));
        assert_eq!(spec.deadline.as_deref(), Some("2026-05-28"));
        assert_eq!(spec.priority.as_deref(), Some("A"));
        assert_eq!(spec.project.as_deref(), Some("Inbox"));
        assert_eq!(spec.description.as_deref(), Some("Follow up"));
    }

    #[test]
    fn keeps_plain_words_as_task_text() {
        let spec = TaskAddSpec::parse(&tokens(&["Call", "Alice", "tag:phone"])).unwrap();
        assert_eq!(spec.source, "pkms");
        assert_eq!(spec.text.as_deref(), Some("Call Alice"));
        assert_eq!(spec.labels, vec!["phone"]);
    }

    #[test]
    fn rejects_duplicate_single_value_options() {
        let err = TaskAddSpec::parse(&tokens(&["title:One", "title:Two"])).unwrap_err();
        assert!(
            err.to_string()
                .contains("title was provided more than once")
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
            parse_add_date_arg("due", "2026-05-27").unwrap(),
            "2026-05-27"
        );
        assert_eq!(
            parse_add_date_arg("due", "2026-05-27 09:30").unwrap(),
            "2026-05-27 09:30"
        );
    }

    #[test]
    fn parses_relative_add_dates_against_explicit_today() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        assert_eq!(
            parse_add_date_arg_on("due", "today", today).unwrap(),
            "2026-05-27"
        );
        assert_eq!(
            parse_add_date_arg_on("due", "tom", today).unwrap(),
            "2026-05-28"
        );
    }
}
