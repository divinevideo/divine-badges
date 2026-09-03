use divine_badges::state::AwardRunStatus;

#[test]
fn award_prepared_status_round_trips_through_storage_text() {
    assert_eq!(AwardRunStatus::AwardPrepared.as_str(), "award_prepared");
    assert_eq!("award_prepared".parse(), Ok(AwardRunStatus::AwardPrepared));
}

#[test]
fn discord_sending_status_round_trips_through_storage_text() {
    assert_eq!(AwardRunStatus::DiscordSending.as_str(), "discord_sending");
    assert_eq!(
        "discord_sending".parse(),
        Ok(AwardRunStatus::DiscordSending)
    );
}
