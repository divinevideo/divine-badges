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
