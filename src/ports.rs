use async_trait::async_trait;

use crate::awards::AwardDefinition;
use crate::error::AppError;
use crate::models::{AwardRun, BadgeDefinitionRecord, CreatorLatestVideo, LeaderboardCreator};
use crate::nostr::DefinitionPublishResult;

#[async_trait(?Send)]
pub trait AwardRepository {
    async fn insert_badge_definition_seed(
        &self,
        record: &BadgeDefinitionRecord,
    ) -> Result<(), AppError>;
    async fn load_badge_definition(
        &self,
        award_slug: &str,
    ) -> Result<Option<BadgeDefinitionRecord>, AppError>;
    async fn save_badge_definition(&self, record: &BadgeDefinitionRecord) -> Result<(), AppError>;
    async fn upsert_award_run(&self, run: AwardRun) -> Result<AwardRun, AppError>;
    async fn save_award_run(&self, run: &AwardRun) -> Result<AwardRun, AppError>;
    async fn load_recent_completed_runs(
        &self,
        award_slug: &str,
        limit: usize,
    ) -> Result<Vec<AwardRun>, AppError>;
    async fn mark_fetch_failed(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_definition_failed(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_award_failed(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_awarded(
        &self,
        award_slug: &str,
        period_key: &str,
        award_event_id: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_discord_pending(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_completed(
        &self,
        award_slug: &str,
        period_key: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_skipped_inactive(
        &self,
        award_slug: &str,
        period_key: &str,
    ) -> Result<AwardRun, AppError>;
}

#[async_trait(?Send)]
pub trait LeaderboardClient {
    async fn ranked_creators(
        &self,
        period: &str,
        candidate_window: usize,
    ) -> Result<Vec<LeaderboardCreator>, AppError>;
}

#[async_trait(?Send)]
pub trait CreatorActivityClient {
    async fn latest_video(&self, pubkey: &str) -> Result<Option<CreatorLatestVideo>, AppError>;
}

#[async_trait(?Send)]
pub trait BadgePublisher {
    async fn publish_definition(
        &self,
        award: &AwardDefinition,
        image_url: &str,
        thumb_url: &str,
    ) -> Result<DefinitionPublishResult, AppError>;

    async fn publish_award(
        &self,
        badge_coordinate: &str,
        winner_pubkey: &str,
        period_key: &str,
    ) -> Result<String, AppError>;
}

#[async_trait(?Send)]
pub trait DiscordClient {
    async fn post_message(&self, message: &str) -> Result<(), AppError>;
}
