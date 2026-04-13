#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwardRunStatus {
    Pending,
    FailedFetch,
    FailedDefinition,
    FailedAward,
    SkippedInactive,
    Awarded,
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
            Self::Awarded => "awarded",
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
            "awarded" => Some(Self::Awarded),
            "awarded_discord_pending" => Some(Self::AwardedDiscordPending),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

pub fn next_status_after_fetch_failure(_current: AwardRunStatus) -> AwardRunStatus {
    AwardRunStatus::FailedFetch
}

pub fn next_status_after_definition_failure(_current: AwardRunStatus) -> AwardRunStatus {
    AwardRunStatus::FailedDefinition
}

pub fn next_status_after_award_failure(_current: AwardRunStatus) -> AwardRunStatus {
    AwardRunStatus::FailedAward
}

pub fn next_status_after_inactive_skip(_current: AwardRunStatus) -> AwardRunStatus {
    AwardRunStatus::SkippedInactive
}

pub fn next_status_after_discord_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Awarded => AwardRunStatus::AwardedDiscordPending,
        other => other,
    }
}
