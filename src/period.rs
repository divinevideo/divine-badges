use chrono::{DateTime, Datelike, Duration, NaiveTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodTarget {
    pub kind: &'static str,
    pub key: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl PeriodTarget {
    pub fn day(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            kind: "day",
            key: start.format("%F").to_string(),
            start,
            end,
        }
    }

    pub fn week(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            kind: "week",
            key: start.format("%G-W%V").to_string(),
            start,
            end,
        }
    }

    pub fn month(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            kind: "month",
            key: start.format("%Y-%m").to_string(),
            start,
            end,
        }
    }
}

pub fn closed_periods_for_tick(now: DateTime<Utc>) -> Vec<PeriodTarget> {
    let end = now.date_naive().and_time(NaiveTime::MIN).and_utc();
    let Some(day_start) = end.checked_sub_signed(Duration::days(1)) else {
        return Vec::new();
    };
    let mut result = vec![PeriodTarget::day(day_start, end)];

    if end.weekday().number_from_monday() == 1 {
        if let Some(week_start) = end.checked_sub_signed(Duration::weeks(1)) {
            result.push(PeriodTarget::week(week_start, end));
        }
    }

    if end.day() == 1 {
        let month_start = day_start.with_day(1);
        if let Some(month_start) = month_start {
            result.push(PeriodTarget::month(month_start, end));
        }
    }

    result
}
