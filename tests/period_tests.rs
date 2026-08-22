use chrono::{DateTime, TimeZone, Utc};
use divine_badges::period::{closed_periods_for_tick, PeriodTarget};

fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

fn period<'a>(periods: &'a [PeriodTarget], kind: &str) -> &'a PeriodTarget {
    periods
        .iter()
        .find(|period| period.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind} period in {periods:?}"))
}

#[test]
fn daily_period_is_the_previous_complete_utc_day() {
    let periods = closed_periods_for_tick(utc(2026, 8, 22, 12, 0, 0));
    let day = period(&periods, "day");

    assert_eq!(day.key, "2026-08-21");
    assert_eq!(day.start, utc(2026, 8, 21, 0, 0, 0));
    assert_eq!(day.end, utc(2026, 8, 22, 0, 0, 0));
}

#[test]
fn daily_boundaries_do_not_change_during_the_tick_day() {
    let just_after_midnight = closed_periods_for_tick(utc(2026, 8, 22, 0, 0, 1));
    let just_before_midnight = closed_periods_for_tick(utc(2026, 8, 22, 23, 59, 59));

    assert_eq!(
        period(&just_after_midnight, "day"),
        period(&just_before_midnight, "day")
    );
}

#[test]
fn monday_processes_the_prior_complete_iso_week() {
    let periods = closed_periods_for_tick(utc(2026, 4, 13, 12, 34, 56));
    let week = period(&periods, "week");

    assert_eq!(week.key, "2026-W15");
    assert_eq!(week.start, utc(2026, 4, 6, 0, 0, 0));
    assert_eq!(week.end, utc(2026, 4, 13, 0, 0, 0));
}

#[test]
fn weekly_boundaries_do_not_change_during_monday() {
    let midnight = closed_periods_for_tick(utc(2026, 4, 13, 0, 0, 0));
    let end_of_day = closed_periods_for_tick(utc(2026, 4, 13, 23, 59, 59));

    assert_eq!(period(&midnight, "week"), period(&end_of_day, "week"));
}

#[test]
fn non_monday_does_not_process_a_week() {
    let periods = closed_periods_for_tick(utc(2026, 4, 14, 12, 0, 0));

    assert!(!periods.iter().any(|period| period.kind == "week"));
}

#[test]
fn period_target_constructors_remain_public() {
    let start = utc(2026, 12, 28, 0, 0, 0);
    let end = utc(2027, 1, 4, 0, 0, 0);

    assert_eq!(PeriodTarget::day(start, end).key, "2026-12-28");
    assert_eq!(PeriodTarget::week(start, end).key, "2026-W53");
    assert_eq!(PeriodTarget::month(start, end).key, "2026-12");
}

#[test]
fn weekly_period_uses_the_prior_weeks_iso_year_at_year_rollover() {
    let periods = closed_periods_for_tick(utc(2027, 1, 4, 12, 0, 0));
    let week = period(&periods, "week");

    assert_eq!(week.key, "2026-W53");
    assert_eq!(week.start, utc(2026, 12, 28, 0, 0, 0));
    assert_eq!(week.end, utc(2027, 1, 4, 0, 0, 0));
}

#[test]
fn first_of_month_processes_the_prior_complete_calendar_month() {
    let periods = closed_periods_for_tick(utc(2026, 3, 1, 18, 45, 0));
    let month = period(&periods, "month");

    assert_eq!(month.key, "2026-02");
    assert_eq!(month.start, utc(2026, 2, 1, 0, 0, 0));
    assert_eq!(month.end, utc(2026, 3, 1, 0, 0, 0));
}

#[test]
fn monthly_period_handles_year_rollover() {
    let periods = closed_periods_for_tick(utc(2026, 1, 1, 12, 0, 0));
    let month = period(&periods, "month");

    assert_eq!(month.key, "2025-12");
    assert_eq!(month.start, utc(2025, 12, 1, 0, 0, 0));
    assert_eq!(month.end, utc(2026, 1, 1, 0, 0, 0));
}

#[test]
fn monthly_period_handles_leap_and_non_leap_february() {
    let leap_periods = closed_periods_for_tick(utc(2024, 3, 1, 12, 0, 0));
    let non_leap_periods = closed_periods_for_tick(utc(2025, 3, 1, 12, 0, 0));

    let leap_month = period(&leap_periods, "month");
    assert_eq!(leap_month.start, utc(2024, 2, 1, 0, 0, 0));
    assert_eq!(leap_month.end, utc(2024, 3, 1, 0, 0, 0));

    let non_leap_month = period(&non_leap_periods, "month");
    assert_eq!(non_leap_month.start, utc(2025, 2, 1, 0, 0, 0));
    assert_eq!(non_leap_month.end, utc(2025, 3, 1, 0, 0, 0));
}

#[test]
fn non_first_of_month_does_not_process_a_month() {
    let periods = closed_periods_for_tick(utc(2026, 3, 2, 12, 0, 0));

    assert!(!periods.iter().any(|period| period.kind == "month"));
}
