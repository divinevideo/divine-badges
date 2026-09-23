use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::awards::AwardDefinition;
use crate::engagement::AutomatedCampaign;
use crate::error::AppError;
use crate::models::{AwardRun, BadgeDefinitionRecord, DiscordDeliveryClaim, DivinerCandidate};
use crate::nostr::{DefinitionPublishResult, SignedNostrEvent};

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
    async fn claim_winner(&self, proposed: &AwardRun) -> Result<AwardRun, AppError>;
    async fn claim_prepared_award(
        &self,
        award_slug: &str,
        period_key: &str,
        proposed: &SignedNostrEvent,
    ) -> Result<AwardRun, AppError>;
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
    async fn mark_preparation_failed(
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
    async fn claim_discord_delivery(
        &self,
        award_slug: &str,
        period_key: &str,
        claim_token: &str,
        now: DateTime<Utc>,
        lease_expires_at: DateTime<Utc>,
    ) -> Result<DiscordDeliveryClaim, AppError>;
    async fn mark_discord_pending(
        &self,
        award_slug: &str,
        period_key: &str,
        claim_token: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_completed(
        &self,
        award_slug: &str,
        period_key: &str,
        claim_token: &str,
    ) -> Result<AwardRun, AppError>;
    async fn mark_skipped_inactive(
        &self,
        award_slug: &str,
        period_key: &str,
    ) -> Result<AwardRun, AppError>;
    async fn claim_push_notification(
        &self,
        award_slug: &str,
        period_key: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, AppError>;
    async fn release_push_notification(
        &self,
        award_slug: &str,
        period_key: &str,
        claimed_at: DateTime<Utc>,
    ) -> Result<(), AppError>;
}

#[async_trait(?Send)]
pub trait DivinerCandidatesClient {
    async fn ranked_candidates(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        candidate_window: usize,
    ) -> Result<Vec<DivinerCandidate>, AppError>;
}

#[async_trait(?Send)]
pub trait BadgePublisher {
    async fn publish_definition(
        &self,
        award: &AwardDefinition,
        image_url: &str,
        thumb_url: &str,
    ) -> Result<DefinitionPublishResult, AppError>;

    fn prepare_award(
        &self,
        badge_coordinate: &str,
        winner_pubkey: &str,
        period_key: &str,
    ) -> Result<SignedNostrEvent, AppError>;

    async fn publish_prepared_award(&self, event: &SignedNostrEvent) -> Result<String, AppError>;
}

#[async_trait(?Send)]
pub trait DiscordClient {
    async fn post_message(
        &self,
        message: &str,
        timeout: std::time::Duration,
    ) -> Result<(), AppError>;
}

#[async_trait(?Send)]
pub trait CampaignClient {
    async fn create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError>;
}
