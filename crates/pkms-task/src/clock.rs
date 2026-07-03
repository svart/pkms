use chrono::{Local, NaiveDate, NaiveTime};

#[derive(Debug, Clone, Copy)]
pub struct TaskClock {
    pub today: NaiveDate,
    pub now: NaiveTime,
}

impl TaskClock {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            today: now.date_naive(),
            now: now.time(),
        }
    }

    pub fn at_start_of_day(today: NaiveDate) -> Self {
        Self {
            today,
            now: NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid"),
        }
    }
}
