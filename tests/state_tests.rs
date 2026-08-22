use divine_badges::state::{
    next_status_after_award_failure, next_status_after_definition_failure,
    next_status_after_discord_failure, next_status_after_fetch_failure,
    next_status_after_inactive_skip, next_status_after_preparation_failure, AwardRunStatus,
};

#[test]
fn discord_failure_after_award_keeps_run_retryable() {
    let next = next_status_after_discord_failure(AwardRunStatus::DiscordSending);
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
        next_status_after_award_failure(AwardRunStatus::AwardPrepared),
        AwardRunStatus::FailedAward
    );
}

#[test]
fn preparation_failure_transitions_to_failed_award() {
    assert_eq!(
        next_status_after_preparation_failure(AwardRunStatus::Pending),
        AwardRunStatus::FailedAward
    );
}

#[test]
fn terminal_status_cannot_regress_through_failure_or_skip_transitions() {
    for transition in [
        next_status_after_fetch_failure,
        next_status_after_definition_failure,
        next_status_after_award_failure,
        next_status_after_inactive_skip,
        next_status_after_preparation_failure,
        next_status_after_discord_failure,
    ] {
        assert_eq!(
            transition(AwardRunStatus::Completed),
            AwardRunStatus::Completed
        );
    }
}

#[test]
fn no_active_creator_transitions_to_skipped_inactive() {
    assert_eq!(
        next_status_after_inactive_skip(AwardRunStatus::Pending),
        AwardRunStatus::SkippedInactive
    );
}

#[test]
fn award_prepared_status_round_trips_through_storage_text() {
    assert_eq!(AwardRunStatus::AwardPrepared.as_str(), "award_prepared");
    assert_eq!(
        AwardRunStatus::from_str("award_prepared"),
        Some(AwardRunStatus::AwardPrepared)
    );
}

#[test]
fn discord_sending_status_round_trips_through_storage_text() {
    assert_eq!(AwardRunStatus::DiscordSending.as_str(), "discord_sending");
    assert_eq!(
        AwardRunStatus::from_str("discord_sending"),
        Some(AwardRunStatus::DiscordSending)
    );
}
