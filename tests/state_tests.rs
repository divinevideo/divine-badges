use divine_badges::state::{
    next_status_after_award_failure, next_status_after_definition_failure,
    next_status_after_discord_failure, next_status_after_fetch_failure,
    next_status_after_inactive_skip, AwardRunStatus,
};

#[test]
fn discord_failure_after_award_keeps_run_retryable() {
    let next = next_status_after_discord_failure(AwardRunStatus::Awarded);
    assert_eq!(next, AwardRunStatus::AwardedDiscordPending);
}

#[test]
fn fetch_failure_transitions_to_failed_fetch() {
    assert_eq!(
        next_status_after_fetch_failure(AwardRunStatus::Pending),
        AwardRunStatus::FailedFetch
    );
}

#[test]
fn definition_failure_transitions_to_failed_definition() {
    assert_eq!(
        next_status_after_definition_failure(AwardRunStatus::Pending),
        AwardRunStatus::FailedDefinition
    );
}

#[test]
fn award_failure_transitions_to_failed_award() {
    assert_eq!(
        next_status_after_award_failure(AwardRunStatus::Pending),
        AwardRunStatus::FailedAward
    );
}

#[test]
fn no_active_creator_transitions_to_skipped_inactive() {
    assert_eq!(
        next_status_after_inactive_skip(AwardRunStatus::Pending),
        AwardRunStatus::SkippedInactive
    );
}
