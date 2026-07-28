use anyhow::{Context, Result};
use chrono::{Datelike, Months, NaiveDate};
use pkms_task::TaskItem;
use std::collections::HashSet;
use std::fmt::Write;

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const BLUE: &str = "\x1b[34m";
const UNDERLINE: &str = "\x1b[4m";
const RED_UNDERLINE: &str = "\x1b[31;4m";
const BLUE_UNDERLINE: &str = "\x1b[34;4m";

pub(super) fn render(
    items: &[TaskItem],
    today: NaiveDate,
    months: i32,
    ansi: bool,
    terminal_width: Option<usize>,
) -> Result<String> {
    let current_month = today
        .with_day(1)
        .context("current month has no first day")?;
    let (first, month_count) = if months < 0 {
        let previous_months = months.unsigned_abs();
        let first = current_month
            .checked_sub_months(Months::new(previous_months))
            .context("calendar month is outside the supported date range")?;
        (first, previous_months as usize + 1)
    } else {
        (current_month, months as usize)
    };
    let visible_end = first
        .checked_add_months(Months::new(
            month_count.try_into().context("month count is too large")?,
        ))
        .and_then(|date| date.pred_opt())
        .context("calendar month is outside the supported date range")?;
    let mut scheduled = task_dates(items, first, visible_end, |item| item.scheduled.as_ref());
    scheduled.extend(
        items
            .iter()
            .filter_map(TaskItem::implicit_daily_file_date)
            .filter_map(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .filter(|date| *date >= first && *date <= visible_end),
    );
    let deadlines = task_dates(items, first, visible_end, |item| item.deadline.as_ref());
    let mut rendered_months = Vec::with_capacity(month_count);

    for offset in 0..month_count {
        let month = first
            .checked_add_months(Months::new(
                offset.try_into().context("month count is too large")?,
            ))
            .context("calendar month is outside the supported date range")?;
        let mut output = String::new();
        render_month(&mut output, month, today, &scheduled, &deadlines, ansi)?;
        rendered_months.push(output);
    }
    let months_per_row = terminal_width
        .map(|width| (width.saturating_add(3) / 23).max(1))
        .unwrap_or(rendered_months.len().max(1));
    let mut output = render_legend(ansi);
    output.push_str("\n\n");
    for (index, row) in rendered_months.chunks(months_per_row).enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(&render_horizontally(row));
    }
    Ok(output)
}

fn render_legend(ansi: bool) -> String {
    if !ansi {
        return "Deadlines  Schedules  Today".to_string();
    }
    format!("{RED}Deadlines{RESET}  {UNDERLINE}Schedules{RESET}  {BLUE}Today{RESET}")
}

fn render_horizontally(months: &[String]) -> String {
    if months.len() == 1 {
        return months[0].clone();
    }

    let lines: Vec<Vec<&str>> = months.iter().map(|month| month.lines().collect()).collect();
    let height = lines.iter().map(Vec::len).max().unwrap_or(0);
    let mut output = String::new();
    for row in 0..height {
        for (index, month) in lines.iter().enumerate() {
            let line = month.get(row).copied().unwrap_or("");
            output.push_str(line);
            if index + 1 < lines.len() {
                for _ in visible_width(line)..23 {
                    output.push(' ');
                }
            }
        }
        output.push('\n');
    }
    output
}

fn visible_width(line: &str) -> usize {
    let mut in_escape = false;
    line.chars()
        .filter(|character| {
            if *character == '\x1b' {
                in_escape = true;
                return false;
            }
            if in_escape {
                if *character == 'm' {
                    in_escape = false;
                }
                return false;
            }
            true
        })
        .count()
}

fn task_dates<'a>(
    items: &'a [TaskItem],
    visible_start: NaiveDate,
    visible_end: NaiveDate,
    select: impl Fn(&'a TaskItem) -> Option<&'a pkms_task::TaskDate>,
) -> HashSet<NaiveDate> {
    let mut dates = HashSet::new();
    for task_date in items.iter().filter_map(select) {
        if let Some(parsed) = pkms_org::org_date::parse_org_date(&task_date.raw) {
            let range_end = parsed
                .base_date_end
                .unwrap_or(parsed.base_date)
                .max(parsed.base_date);
            let mut date = parsed.base_date.max(visible_start);
            let end = range_end.min(visible_end);
            if date > end {
                continue;
            }
            loop {
                dates.insert(date);
                if date >= end {
                    break;
                }
                let Some(next) = date.succ_opt() else {
                    break;
                };
                date = next;
            }
        } else if let Some(date) = task_date
            .date
            .as_ref()
            .and_then(pkms_task::TaskDateValue::parse_naive_date)
            .filter(|date| *date >= visible_start && *date <= visible_end)
        {
            dates.insert(date);
        }
    }
    dates
}

fn render_month(
    output: &mut String,
    month: NaiveDate,
    today: NaiveDate,
    scheduled: &HashSet<NaiveDate>,
    deadlines: &HashSet<NaiveDate>,
    ansi: bool,
) -> Result<()> {
    writeln!(output, "{:^20}", month.format("%B %Y"))?;
    writeln!(output, "Mo Tu We Th Fr Sa Su")?;

    let leading_days = month.weekday().num_days_from_monday();
    for _ in 0..leading_days {
        output.push_str("   ");
    }

    let mut date = month;
    loop {
        let day = format!("{:>2}", date.day());
        let styled = style_day(
            &day,
            scheduled.contains(&date),
            deadlines.contains(&date),
            date == today,
            ansi,
        );
        output.push_str(&styled);

        let next = date
            .succ_opt()
            .context("calendar date is outside supported range")?;
        let is_last_day = next.month() != month.month();
        if date.weekday().num_days_from_monday() == 6 || is_last_day {
            output.push('\n');
        } else {
            output.push(' ');
        }

        if is_last_day {
            break;
        }
        date = next;
    }
    Ok(())
}

fn style_day(day: &str, scheduled: bool, deadline: bool, today: bool, ansi: bool) -> String {
    if !ansi || (!scheduled && !deadline && !today) {
        return day.to_string();
    }
    let prefix = match (scheduled, deadline, today) {
        (true, true, _) => RED_UNDERLINE,
        (false, true, _) => RED,
        (true, false, true) => BLUE_UNDERLINE,
        (false, false, true) => BLUE,
        (true, false, false) => UNDERLINE,
        (false, false, false) => unreachable!(),
    };
    format!("{prefix}{day}{RESET}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkms_task::{TaskDate, TaskDateValue, TaskId, TaskSourceKind, TaskStatus};

    fn task(scheduled: Option<&str>, deadline: Option<&str>) -> TaskItem {
        TaskItem {
            id: TaskId::Pkms(1),
            display_id: "1".to_string(),
            source: TaskSourceKind::Pkms,
            source_id: "1".to_string(),
            title: "Calendar task".to_string(),
            body: None,
            status: TaskStatus::Open,
            state: None,
            priority: None,
            scheduled: scheduled.map(|date| TaskDate {
                raw: date.to_string(),
                date: Some(TaskDateValue::new(date)),
            }),
            deadline: deadline.map(|date| TaskDate {
                raw: date.to_string(),
                date: Some(TaskDateValue::new(date)),
            }),
            tags: Vec::new(),
            project: None,
            project_id: None,
            note_title: None,
            note_uuid: None,
            path: None,
            has_agenda_tag: None,
            is_daily_file: false,
            daily_file_date: None,
            heading_level: None,
            line_number: None,
            url: None,
            is_overdue: false,
        }
    }

    #[test]
    fn styles_scheduled_deadline_and_combined_days() {
        assert_eq!(
            style_day(" 1", true, false, false, true),
            "\x1b[4m 1\x1b[0m"
        );
        assert_eq!(
            style_day(" 2", false, true, false, true),
            "\x1b[31m 2\x1b[0m"
        );
        assert_eq!(
            style_day(" 3", true, true, true, true),
            "\x1b[31;4m 3\x1b[0m"
        );
        assert_eq!(style_day(" 3", true, true, true, false), " 3");
    }

    #[test]
    fn styles_current_day_blue_unless_it_has_a_deadline() {
        assert_eq!(
            style_day("15", false, false, true, true),
            "\x1b[34m15\x1b[0m"
        );
        assert_eq!(
            style_day("15", true, false, true, true),
            "\x1b[34;4m15\x1b[0m"
        );
        assert_eq!(
            style_day("15", false, true, true, true),
            "\x1b[31m15\x1b[0m"
        );
    }

    #[test]
    fn renders_monday_first_month_grid() {
        let output = render(
            &[],
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            1,
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            output,
            "Deadlines  Schedules  Today\n\n     July 2026      \nMo Tu We Th Fr Sa Su\n       1  2  3  4  5\n 6  7  8  9 10 11 12\n13 14 15 16 17 18 19\n20 21 22 23 24 25 26\n27 28 29 30 31\n"
        );
    }

    #[test]
    fn styles_calendar_legend_when_ansi_is_enabled() {
        let output = render(
            &[],
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            1,
            true,
            None,
        )
        .unwrap();

        assert!(output.starts_with(
            "\x1b[31mDeadlines\x1b[0m  \x1b[4mSchedules\x1b[0m  \x1b[34mToday\x1b[0m\n\n"
        ));
    }

    #[test]
    fn renders_consecutive_months() {
        let output = render(
            &[],
            NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
            2,
            false,
            Some(80),
        )
        .unwrap();

        assert!(output.contains("December 2026"));
        assert!(output.contains("January 2027"));
        assert_eq!(output.matches("Mo Tu We Th Fr Sa Su").count(), 2);
        assert!(
            output
                .lines()
                .find(|line| line.contains("December 2026"))
                .is_some_and(|line| line.contains("December 2026") && line.contains("January 2027")),
            "output:\n{output}"
        );
    }

    #[test]
    fn wraps_months_to_new_rows_at_terminal_width() {
        let output = render(
            &[],
            NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
            3,
            false,
            Some(50),
        )
        .unwrap();
        let january_line = output
            .lines()
            .find(|line| line.contains("January 2027"))
            .unwrap();
        let february_line = output
            .lines()
            .find(|line| line.contains("February 2027"))
            .unwrap();

        assert!(january_line.contains("December 2026"));
        assert!(!february_line.contains("January 2027"));
    }

    #[test]
    fn negative_month_count_includes_previous_months_and_current_month() {
        let output = render(
            &[],
            NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
            -1,
            false,
            Some(80),
        )
        .unwrap();
        let heading = output
            .lines()
            .find(|line| line.contains("December 2026"))
            .unwrap();

        assert!(heading.contains("December 2026"));
        assert!(heading.contains("January 2027"));
        assert!(!output.contains("February 2027"));
    }

    #[test]
    fn styles_dates_from_task_schedule_and_deadline() {
        let items = [task(Some("2026-07-08"), Some("2026-07-10"))];

        let output = render(
            &items,
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            1,
            true,
            None,
        )
        .unwrap();

        assert!(output.contains("\x1b[4m 8\x1b[0m"));
        assert!(output.contains("\x1b[31m10\x1b[0m"));
    }

    #[test]
    fn styles_every_day_in_scheduled_and_deadline_ranges() {
        let mut item = task(Some("2026-06-20"), Some("2026-06-24"));
        item.scheduled.as_mut().unwrap().raw = "<2026-06-20 Sat>--<2026-06-22 Mon>".to_string();
        item.deadline.as_mut().unwrap().raw = "<2026-06-24 Wed>--<2026-06-26 Fri>".to_string();

        let output = render(
            &[item],
            NaiveDate::from_ymd_opt(2026, 7, 17).unwrap(),
            -1,
            true,
            None,
        )
        .unwrap();

        for day in [20, 21, 22] {
            assert!(
                output.contains(&format!("\x1b[4m{day}\x1b[0m")),
                "scheduled day {day} was not underlined:\n{output}"
            );
        }
        for day in [24, 25, 26] {
            assert!(
                output.contains(&format!("\x1b[31m{day}\x1b[0m")),
                "deadline day {day} was not colored:\n{output}"
            );
        }
    }

    #[test]
    fn underlines_unscheduled_task_on_its_daily_file_date() {
        let mut item = task(None, None);
        item.is_daily_file = true;
        item.daily_file_date = Some(TaskDateValue::new("2026-06-22"));

        let output = render(
            &[item],
            NaiveDate::from_ymd_opt(2026, 7, 17).unwrap(),
            -1,
            true,
            None,
        )
        .unwrap();

        assert!(
            output.contains("\x1b[4m22\x1b[0m"),
            "daily task date was not underlined:\n{output}"
        );
    }
}
