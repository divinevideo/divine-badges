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

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "failed_fetch" => Some(Self::FailedFetch),
            "failed_definition" => Some(Self::FailedDefinition),
            "failed_award" => Some(Self::FailedAward),
            "skipped_inactive" => Some(Self::SkippedInactive),
            "award_prepared" => Some(Self::AwardPrepared),
            "awarded" => Some(Self::Awarded),
            "discord_sending" => Some(Self::DiscordSending),
            "awarded_discord_pending" => Some(Self::AwardedDiscordPending),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

pub fn next_status_after_fetch_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Pending | AwardRunStatus::FailedFetch | AwardRunStatus::SkippedInactive => {
            AwardRunStatus::FailedFetch
        }
        other => other,
    }
}

pub fn next_status_after_definition_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Pending
        | AwardRunStatus::FailedFetch
        | AwardRunStatus::FailedDefinition
        | AwardRunStatus::SkippedInactive => AwardRunStatus::FailedDefinition,
        other => other,
    }
}

pub fn next_status_after_award_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::AwardPrepared | AwardRunStatus::FailedAward => AwardRunStatus::FailedAward,
        other => other,
    }
}

pub fn next_status_after_preparation_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Pending
        | AwardRunStatus::FailedDefinition
        | AwardRunStatus::FailedAward => AwardRunStatus::FailedAward,
        other => other,
    }
}

pub fn next_status_after_inactive_skip(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Pending | AwardRunStatus::FailedFetch | AwardRunStatus::SkippedInactive => {
            AwardRunStatus::SkippedInactive
        }
        other => other,
    }
}

pub fn next_status_after_discord_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::DiscordSending => AwardRunStatus::AwardedDiscordPending,
        other => other,
    }
}
