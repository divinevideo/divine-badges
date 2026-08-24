#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwardRunStatus {
    Pending,
    FailedFetch,
    FailedDefinition,
    FailedAward,
    SkippedInactive,
    AwardPrepared,
    Awarded,
    DiscordSending,
    AwardedDiscordPending,
    Completed,
}

impl AwardRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::FailedFetch => "failed_fetch",
            Self::FailedDefinition => "failed_definition",
            Self::FailedAward => "failed_award",
            Self::SkippedInactive => "skipped_inactive",
            Self::AwardPrepared => "award_prepared",
            Self::Awarded => "awarded",
            Self::DiscordSending => "discord_sending",
            Self::AwardedDiscordPending => "awarded_discord_pending",
            Self::Completed => "completed",
        }
    }
}

impl std::str::FromStr for AwardRunStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "failed_fetch" => Ok(Self::FailedFetch),
            "failed_definition" => Ok(Self::FailedDefinition),
            "failed_award" => Ok(Self::FailedAward),
            "skipped_inactive" => Ok(Self::SkippedInactive),
            "award_prepared" => Ok(Self::AwardPrepared),
            "awarded" => Ok(Self::Awarded),
            "discord_sending" => Ok(Self::DiscordSending),
            "awarded_discord_pending" => Ok(Self::AwardedDiscordPending),
            "completed" => Ok(Self::Completed),
            _ => Err(()),
        }
    }
}
