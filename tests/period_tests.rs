use chrono::{TimeZone, Utc};
use divine_badges::period::{closed_periods_for_tick, PeriodTarget};

#[test]
fn monday_after_midnight_processes_day_and_week() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let periods = closed_periods_for_tick(now);

    assert!(periods.contains(&PeriodTarget::day("2026-04-12")));
    assert!(periods.contains(&PeriodTarget::week("2026-W15")));
}
