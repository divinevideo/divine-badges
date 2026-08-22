use divine_badges::state::AwardRunStatus;

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
