use chrono::{DateTime, Datelike, Duration, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodTarget {
    pub kind: &'static str,
    pub key: String,
}

impl PeriodTarget {
    pub fn day(key: &str) -> Self {
        Self {
            kind: "day",
            key: key.to_string(),
        }
    }

    pub fn week(key: &str) -> Self {
        Self {
            kind: "week",
            key: key.to_string(),
        }
    }

    pub fn month(key: &str) -> Self {
        Self {
            kind: "month",
            key: key.to_string(),
        }
    }
}

pub fn closed_periods_for_tick(now: DateTime<Utc>) -> Vec<PeriodTarget> {
    let previous_day = now - Duration::days(1);
    let mut result = vec![PeriodTarget::day(&previous_day.format("%F").to_string())];

    if now.weekday().number_from_monday() == 1 {
        result.push(PeriodTarget::week(&format!(
            "{:04}-W{:02}",
            previous_day.iso_week().year(),
            previous_day.iso_week().week()
        )));
    }

    if now.day() == 1 {
        result.push(PeriodTarget::month(
            &previous_day.format("%Y-%m").to_string(),
        ));
    }

    result
}
