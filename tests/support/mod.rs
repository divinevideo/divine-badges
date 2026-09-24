use divine_badges::awards::award_for_period_kind;
use divine_badges::models::AwardRun;
use divine_badges::state::AwardRunStatus;

/// Build a completed Diviner-of-the-Day run for one winner.
pub fn completed_run(display_name: Option<&str>, winner_pubkey: Option<&str>) -> AwardRun {
    let award = award_for_period_kind("day").unwrap();
    let mut run = AwardRun::pending(award.slug, "2026-09-22", "day");
    run.status = AwardRunStatus::Completed;
    run.discord_message_sent = true;
    run.winner_pubkey = winner_pubkey.map(str::to_string);
    run.winner_display_name = display_name.map(str::to_string);
    run
}
