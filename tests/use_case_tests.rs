use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use divine_badges::awards::award_for_period_kind;
use divine_badges::clock::Clock;
use divine_badges::config::AppConfig;
use divine_badges::divine_api::ranked_candidates_for_period;
use divine_badges::eligibility::DIVINER_AWARD_EXCLUDED_PUBKEYS;
use divine_badges::engagement::AutomatedCampaign;
use divine_badges::error::AppError;
use divine_badges::models::{
    AwardRun, BadgeDefinitionRecord, DiscordDeliveryClaim, DivinerCandidate,
};
use divine_badges::nostr::{DefinitionPublishResult, SignedNostrEvent};
use divine_badges::ports::{
    AwardRepository, BadgePublisher, CampaignClient, DiscordClient, DivinerCandidatesClient,
};
use divine_badges::state::AwardRunStatus;
use divine_badges::use_cases::{run_award_tick_with_clock, TickOutcome};
use futures::executor::block_on;

const FOUNDER: &str = "d95aa8fc0eff8e488952495b8064991d27fb96ed8652f12cdedc5a4e8b5ae540";
const FIRST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SECOND: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const JS_SAFE_MAX: u64 = 9_007_199_254_740_991;
type CandidateCall = (DateTime<Utc>, DateTime<Utc>, usize);

struct FakeRepo {
    definitions: RefCell<HashMap<String, BadgeDefinitionRecord>>,
    runs: RefCell<HashMap<(String, String), AwardRun>>,
    operations: Rc<RefCell<Vec<String>>>,
    winner_claim_override: RefCell<Option<AwardRun>>,
    prepared_claim_override: RefCell<Option<SignedNostrEvent>>,
    fail_mark_awarded_once: RefCell<bool>,
    complete_before_mark_awarded: RefCell<bool>,
    complete_before_mark_award_failed: RefCell<bool>,
    prepare_before_mark_preparation_failed: RefCell<bool>,
    discord_claim_times: RefCell<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
    push_notified: RefCell<HashSet<(String, String)>>,
}

impl Default for FakeRepo {
    fn default() -> Self {
        Self {
            definitions: RefCell::new(HashMap::new()),
            runs: RefCell::new(HashMap::new()),
            operations: Rc::new(RefCell::new(Vec::new())),
            winner_claim_override: RefCell::new(None),
            prepared_claim_override: RefCell::new(None),
            fail_mark_awarded_once: RefCell::new(false),
            complete_before_mark_awarded: RefCell::new(false),
            complete_before_mark_award_failed: RefCell::new(false),
            prepare_before_mark_preparation_failed: RefCell::new(false),
            discord_claim_times: RefCell::new(Vec::new()),
            push_notified: RefCell::new(HashSet::new()),
        }
    }
}

impl FakeRepo {
    fn update(
        &self,
        award_slug: &str,
        period_key: &str,
        status: AwardRunStatus,
        error: Option<&str>,
    ) -> Result<AwardRun, AppError> {
        let mut runs = self.runs.borrow_mut();
        let run = runs
            .get_mut(&(award_slug.into(), period_key.into()))
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        run.status = status;
        run.error_message = error.map(str::to_owned);
        Ok(run.clone())
    }

    fn store(&self, run: AwardRun) -> AwardRun {
        self.runs.borrow_mut().insert(
            (run.award_slug.clone(), run.period_key.clone()),
            run.clone(),
        );
        run
    }
}

fn copy_winner_receipt(target: &mut AwardRun, source: &AwardRun) {
    target.winner_pubkey = source.winner_pubkey.clone();
    target.winner_display_name = source.winner_display_name.clone();
    target.winner_name = source.winner_name.clone();
    target.winner_nip05 = source.winner_nip05.clone();
    target.winner_picture = source.winner_picture.clone();
    target.latest_eligible_publication_at = source.latest_eligible_publication_at;
    target.loops = source.loops;
    target.views = source.views;
    target.unique_viewers = source.unique_viewers;
    target.videos_with_views = source.videos_with_views;
    target.positive_reactors = source.positive_reactors;
    target.distinct_commenters = source.distinct_commenters;
    target.distinct_reposters = source.distinct_reposters;
    target.distinct_positive_engagers = source.distinct_positive_engagers;
    target.engagement_tier = source.engagement_tier;
    target.engagement_rate = source.engagement_rate;
    target.score = source.score;
}

#[async_trait(?Send)]
impl AwardRepository for FakeRepo {
    async fn insert_badge_definition_seed(
        &self,
        record: &BadgeDefinitionRecord,
    ) -> Result<(), AppError> {
        self.definitions
            .borrow_mut()
            .entry(record.award_slug.clone())
            .or_insert_with(|| record.clone());
        Ok(())
    }

    async fn load_badge_definition(
        &self,
        award_slug: &str,
    ) -> Result<Option<BadgeDefinitionRecord>, AppError> {
        Ok(self.definitions.borrow().get(award_slug).cloned())
    }

    async fn save_badge_definition(&self, record: &BadgeDefinitionRecord) -> Result<(), AppError> {
        self.definitions
            .borrow_mut()
            .insert(record.award_slug.clone(), record.clone());
        Ok(())
    }

    async fn upsert_award_run(&self, run: AwardRun) -> Result<AwardRun, AppError> {
        let key = (run.award_slug.clone(), run.period_key.clone());
        Ok(self.runs.borrow_mut().entry(key).or_insert(run).clone())
    }

    async fn claim_winner(&self, proposed: &AwardRun) -> Result<AwardRun, AppError> {
        self.operations.borrow_mut().push("claim-winner".into());
        let key = (proposed.award_slug.clone(), proposed.period_key.clone());
        let current = self
            .runs
            .borrow()
            .get(&key)
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_some()
            || !matches!(
                current.status,
                AwardRunStatus::Pending
                    | AwardRunStatus::FailedFetch
                    | AwardRunStatus::SkippedInactive
            )
        {
            return Ok(current);
        }

        let source = self
            .winner_claim_override
            .borrow_mut()
            .take()
            .unwrap_or_else(|| proposed.clone());
        let mut claimed = current;
        copy_winner_receipt(&mut claimed, &source);
        claimed.status = AwardRunStatus::Pending;
        claimed.error_message = None;
        Ok(self.store(claimed))
    }

    async fn claim_prepared_award(
        &self,
        award_slug: &str,
        period_key: &str,
        proposed: &SignedNostrEvent,
    ) -> Result<AwardRun, AppError> {
        self.operations.borrow_mut().push("claim-prepared".into());
        let key = (award_slug.to_string(), period_key.to_string());
        let current = self
            .runs
            .borrow()
            .get(&key)
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_none()
            || current.prepared_award_event.is_some()
            || matches!(
                current.status,
                AwardRunStatus::AwardPrepared
                    | AwardRunStatus::Awarded
                    | AwardRunStatus::DiscordSending
                    | AwardRunStatus::AwardedDiscordPending
                    | AwardRunStatus::Completed
            )
        {
            return Ok(current);
        }

        let event = self
            .prepared_claim_override
            .borrow_mut()
            .take()
            .unwrap_or_else(|| proposed.clone());
        let mut claimed = current;
        claimed.prepared_award_event = Some(serde_json::to_string(&event).unwrap());
        claimed.award_event_id = Some(event.id.clone());
        claimed.status = AwardRunStatus::AwardPrepared;
        claimed.error_message = None;
        Ok(self.store(claimed))
    }

    async fn load_recent_completed_runs(
        &self,
        award_slug: &str,
        limit: usize,
    ) -> Result<Vec<AwardRun>, AppError> {
        let mut runs = self
            .runs
            .borrow()
            .values()
            .filter(|run| run.award_slug == award_slug && run.status == AwardRunStatus::Completed)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| right.period_key.cmp(&left.period_key));
        runs.truncate(limit);
        Ok(runs)
    }

    async fn mark_fetch_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_some()
            || current.prepared_award_event.is_some()
            || !matches!(
                current.status,
                AwardRunStatus::Pending
                    | AwardRunStatus::FailedFetch
                    | AwardRunStatus::SkippedInactive
            )
        {
            return Ok(current);
        }
        self.update(slug, key, AwardRunStatus::FailedFetch, Some(error))
    }

    async fn mark_definition_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_none()
            || current.prepared_award_event.is_some()
            || !matches!(
                current.status,
                AwardRunStatus::Pending | AwardRunStatus::FailedDefinition
            )
        {
            return Ok(current);
        }
        self.update(slug, key, AwardRunStatus::FailedDefinition, Some(error))
    }

    async fn mark_preparation_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        if *self.prepare_before_mark_preparation_failed.borrow() {
            let mut prepared = self
                .runs
                .borrow()
                .get(&(slug.into(), key.into()))
                .cloned()
                .unwrap();
            prepared.prepared_award_event = Some("concurrent signed event".into());
            prepared.award_event_id = Some("concurrent-event-id".into());
            prepared.status = AwardRunStatus::AwardPrepared;
            self.store(prepared);
        }
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_none()
            || current.prepared_award_event.is_some()
            || !matches!(
                current.status,
                AwardRunStatus::Pending
                    | AwardRunStatus::FailedDefinition
                    | AwardRunStatus::FailedAward
            )
        {
            return Ok(current);
        }
        self.update(slug, key, AwardRunStatus::FailedAward, Some(error))
    }

    async fn mark_award_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        if *self.complete_before_mark_award_failed.borrow() {
            let mut completed = self
                .runs
                .borrow()
                .get(&(slug.into(), key.into()))
                .cloned()
                .unwrap();
            completed.status = AwardRunStatus::Completed;
            completed.discord_message_sent = true;
            self.store(completed);
        }
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.prepared_award_event.is_none()
            || !matches!(
                current.status,
                AwardRunStatus::AwardPrepared | AwardRunStatus::FailedAward
            )
        {
            return Ok(current);
        }
        self.update(slug, key, AwardRunStatus::FailedAward, Some(error))
    }

    async fn mark_awarded(
        &self,
        slug: &str,
        key: &str,
        event_id: &str,
    ) -> Result<AwardRun, AppError> {
        if *self.fail_mark_awarded_once.borrow() {
            *self.fail_mark_awarded_once.borrow_mut() = false;
            return Err(AppError::Repository(
                "simulated crash after relay acceptance".into(),
            ));
        }
        if *self.complete_before_mark_awarded.borrow() {
            let mut completed = self
                .runs
                .borrow()
                .get(&(slug.into(), key.into()))
                .cloned()
                .unwrap();
            completed.status = AwardRunStatus::Completed;
            completed.discord_message_sent = true;
            self.store(completed);
        }
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.prepared_award_event.is_none()
            || current.award_event_id.as_deref() != Some(event_id)
            || !matches!(
                current.status,
                AwardRunStatus::AwardPrepared | AwardRunStatus::FailedAward
            )
        {
            return Ok(current);
        }
        let mut run = self.update(slug, key, AwardRunStatus::Awarded, None)?;
        run.award_event_id = Some(event_id.into());
        Ok(self.store(run))
    }

    async fn claim_discord_delivery(
        &self,
        slug: &str,
        key: &str,
        claim_token: &str,
        now: DateTime<Utc>,
        lease_expires_at: DateTime<Utc>,
    ) -> Result<DiscordDeliveryClaim, AppError> {
        self.discord_claim_times
            .borrow_mut()
            .push((now, lease_expires_at));
        let mut current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        let available = !current.discord_message_sent
            && (matches!(
                current.status,
                AwardRunStatus::Awarded | AwardRunStatus::AwardedDiscordPending
            ) || (current.status == AwardRunStatus::DiscordSending
                && current
                    .discord_lease_expires_at
                    .map(|expires| expires <= now)
                    .unwrap_or(false)));
        if available {
            current.status = AwardRunStatus::DiscordSending;
            current.discord_claim_token = Some(claim_token.into());
            current.discord_lease_expires_at = Some(lease_expires_at);
            current.error_message = None;
            current = self.store(current);
        }
        Ok(DiscordDeliveryClaim {
            acquired: current.status == AwardRunStatus::DiscordSending
                && current.discord_claim_token.as_deref() == Some(claim_token),
            run: current,
        })
    }

    async fn mark_discord_pending(
        &self,
        slug: &str,
        key: &str,
        claim_token: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        let mut current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.status == AwardRunStatus::DiscordSending
            && current.discord_claim_token.as_deref() == Some(claim_token)
        {
            current.status = AwardRunStatus::AwardedDiscordPending;
            current.discord_claim_token = None;
            current.discord_lease_expires_at = None;
            current.error_message = Some(error.into());
            current = self.store(current);
        }
        Ok(current)
    }

    async fn mark_completed(
        &self,
        slug: &str,
        key: &str,
        claim_token: &str,
    ) -> Result<AwardRun, AppError> {
        let mut current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.status == AwardRunStatus::DiscordSending
            && current.discord_claim_token.as_deref() == Some(claim_token)
        {
            current.status = AwardRunStatus::Completed;
            current.discord_message_sent = true;
            current.discord_claim_token = None;
            current.discord_lease_expires_at = None;
            current.error_message = None;
            current = self.store(current);
        }
        Ok(current)
    }

    async fn mark_skipped_inactive(&self, slug: &str, key: &str) -> Result<AwardRun, AppError> {
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.winner_pubkey.is_some()
            || current.prepared_award_event.is_some()
            || !matches!(
                current.status,
                AwardRunStatus::Pending
                    | AwardRunStatus::FailedFetch
                    | AwardRunStatus::SkippedInactive
            )
        {
            return Ok(current);
        }
        self.update(slug, key, AwardRunStatus::SkippedInactive, None)
    }

    async fn claim_push_notification(
        &self,
        slug: &str,
        key: &str,
        _now: DateTime<Utc>,
    ) -> Result<bool, AppError> {
        let current = self
            .runs
            .borrow()
            .get(&(slug.into(), key.into()))
            .cloned()
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        if current.status != AwardRunStatus::Completed {
            return Ok(false);
        }
        Ok(self
            .push_notified
            .borrow_mut()
            .insert((slug.to_string(), key.to_string())))
    }

    async fn release_push_notification(
        &self,
        slug: &str,
        key: &str,
        _claimed_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        self.push_notified
            .borrow_mut()
            .remove(&(slug.to_string(), key.to_string()));
        Ok(())
    }
}

enum CandidateFailure {
    Empty,
    Api(String),
}

#[derive(Default)]
struct FakeCandidates {
    candidates: Vec<DivinerCandidate>,
    failure: Option<CandidateFailure>,
    calls: RefCell<Vec<CandidateCall>>,
}

#[async_trait(?Send)]
impl DivinerCandidatesClient for FakeCandidates {
    async fn ranked_candidates(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<DivinerCandidate>, AppError> {
        self.calls.borrow_mut().push((start, end, limit));
        match &self.failure {
            Some(CandidateFailure::Empty) => Err(AppError::EmptyCandidates(format!(
                "{}..{}",
                start.to_rfc3339(),
                end.to_rfc3339()
            ))),
            Some(CandidateFailure::Api(error)) => Err(AppError::Api(error.clone())),
            None => Ok(self.candidates.clone()),
        }
    }
}

struct FakePublisher {
    count: RefCell<usize>,
    prepare_count: RefCell<u64>,
    published_awards: RefCell<Vec<SignedNostrEvent>>,
    operations: Rc<RefCell<Vec<String>>>,
    fail_definition: bool,
    fail_prepare: bool,
    fail_award: bool,
}

impl FakePublisher {
    fn new(operations: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            count: RefCell::new(0),
            prepare_count: RefCell::new(0),
            published_awards: RefCell::new(Vec::new()),
            operations,
            fail_definition: false,
            fail_prepare: false,
            fail_award: false,
        }
    }
}

#[async_trait(?Send)]
impl BadgePublisher for FakePublisher {
    async fn publish_definition(
        &self,
        award: &divine_badges::awards::AwardDefinition,
        _image_url: &str,
        _thumb_url: &str,
    ) -> Result<DefinitionPublishResult, AppError> {
        *self.count.borrow_mut() += 1;
        self.operations
            .borrow_mut()
            .push("publish-definition".into());
        if self.fail_definition {
            return Err(AppError::Relay("definition unavailable".into()));
        }
        Ok(DefinitionPublishResult {
            definition_event_id: format!("definition-{}", award.slug),
            definition_coordinate: format!("30009:issuerpubkey:{}", award.d_tag),
        })
    }

    fn prepare_award(
        &self,
        coordinate: &str,
        pubkey: &str,
        period_key: &str,
    ) -> Result<SignedNostrEvent, AppError> {
        if self.fail_prepare {
            return Err(AppError::Relay("signing unavailable".into()));
        }
        *self.prepare_count.borrow_mut() += 1;
        Ok(signed_event(
            &format!("prepared-{}", self.prepare_count.borrow()),
            coordinate,
            pubkey,
            period_key,
        ))
    }

    async fn publish_prepared_award(&self, event: &SignedNostrEvent) -> Result<String, AppError> {
        *self.count.borrow_mut() += 1;
        self.published_awards.borrow_mut().push(event.clone());
        self.operations.borrow_mut().push("publish-prepared".into());
        if self.fail_award {
            return Err(AppError::Relay("award relay unavailable".into()));
        }
        Ok(event.id.clone())
    }
}

#[derive(Default)]
struct FakeDiscord {
    messages: RefCell<Vec<String>>,
    failure: RefCell<Option<String>>,
    timeouts: RefCell<Vec<std::time::Duration>>,
}

#[async_trait(?Send)]
impl DiscordClient for FakeDiscord {
    async fn post_message(
        &self,
        message: &str,
        timeout: std::time::Duration,
    ) -> Result<(), AppError> {
        self.messages.borrow_mut().push(message.into());
        self.timeouts.borrow_mut().push(timeout);
        match self.failure.borrow().as_deref() {
            Some(error) => Err(AppError::Discord(error.into())),
            None => Ok(()),
        }
    }
}

#[derive(Default)]
struct FakeCampaignClient {
    created: RefCell<Vec<AutomatedCampaign>>,
    failure: RefCell<bool>,
}

impl FakeCampaignClient {
    fn failing() -> Self {
        Self {
            created: RefCell::new(Vec::new()),
            failure: RefCell::new(true),
        }
    }
}

#[async_trait(?Send)]
impl CampaignClient for FakeCampaignClient {
    async fn create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError> {
        if *self.failure.borrow() {
            return Err(AppError::Api("engagement unavailable".into()));
        }
        self.created.borrow_mut().push(campaign.clone());
        Ok(())
    }
}

struct InFlightReclaimDiscord<'a> {
    repo: &'a FakeRepo,
    claim_time: DateTime<Utc>,
    timeout: RefCell<Option<std::time::Duration>>,
    reclaim_acquired: RefCell<Option<bool>>,
}

#[async_trait(?Send)]
impl DiscordClient for InFlightReclaimDiscord<'_> {
    async fn post_message(
        &self,
        _message: &str,
        timeout: std::time::Duration,
    ) -> Result<(), AppError> {
        *self.timeout.borrow_mut() = Some(timeout);
        let reclaim_time = self.claim_time + Duration::from_std(timeout).unwrap();
        let competing_claim = self
            .repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "competing-worker",
                reclaim_time,
                reclaim_time + Duration::minutes(5),
            )
            .await?;
        *self.reclaim_acquired.borrow_mut() = Some(competing_claim.acquired);
        Ok(())
    }
}

fn config() -> AppConfig {
    AppConfig {
        divine_api_base_url: "https://api.divine.video".into(),
        divine_relay_url: "wss://relay.divine.video".into(),
        nostr_issuer_nsec: "nsec1example".into(),
        discord_webhook_url: "https://discord.example/webhook".into(),
        divine_badge_image_url: "https://cdn.divine.video/logo.png".into(),
        divine_creator_base_url: "https://divine.video".into(),
        engagement_api_base_url: None,
        engagement_access_client_id: None,
        engagement_access_client_secret: None,
    }
}

fn config_with_engagement() -> AppConfig {
    AppConfig {
        engagement_api_base_url: Some("https://engagement.example".into()),
        engagement_access_client_id: Some("access-id".into()),
        engagement_access_client_secret: Some("access-secret".into()),
        ..config()
    }
}

fn period_end() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()
}

fn candidate(pubkey: &str, display_name: &str, rank: u64) -> DivinerCandidate {
    DivinerCandidate {
        pubkey: pubkey.into(),
        name: display_name.into(),
        display_name: display_name.into(),
        nip05: None,
        picture: "https://cdn.divine.video/creator.png".into(),
        latest_eligible_publication_at: period_end() - Duration::hours(12),
        views: 30,
        unique_viewers: 20,
        loops: 1.5,
        videos_with_views: 2,
        positive_reactors: 7,
        distinct_commenters: 5,
        distinct_reposters: 3,
        distinct_positive_engagers: 10,
        engagement_tier: 1,
        engagement_rate: 0.5,
        score: 88.25,
        rank,
    }
}

fn run_with_winner(pubkey: &str, display_name: &str) -> AwardRun {
    let award = award_for_period_kind("day").unwrap();
    let mut run = AwardRun::pending(award.slug, "2026-04-14", "day");
    let candidate = candidate(pubkey, display_name, 1);
    run.winner_display_name = Some(candidate.best_display_name());
    run.winner_pubkey = Some(candidate.pubkey.clone());
    run.winner_name = Some(candidate.name);
    run.winner_nip05 = candidate.nip05;
    run.winner_picture = Some(candidate.picture);
    run.latest_eligible_publication_at = Some(candidate.latest_eligible_publication_at);
    run.loops = Some(candidate.loops);
    run.views = Some(candidate.views as i64);
    run.unique_viewers = Some(candidate.unique_viewers as i64);
    run.videos_with_views = Some(candidate.videos_with_views as i64);
    run.positive_reactors = Some(candidate.positive_reactors as i64);
    run.distinct_commenters = Some(candidate.distinct_commenters as i64);
    run.distinct_reposters = Some(candidate.distinct_reposters as i64);
    run.distinct_positive_engagers = Some(candidate.distinct_positive_engagers as i64);
    run.engagement_tier = Some(i64::from(candidate.engagement_tier));
    run.engagement_rate = Some(candidate.engagement_rate);
    run.score = Some(candidate.score);
    run
}

fn signed_event(
    id_seed: &str,
    coordinate: &str,
    pubkey: &str,
    period_key: &str,
) -> SignedNostrEvent {
    SignedNostrEvent {
        id: format!("{id_seed:0<64}"),
        pubkey: "f".repeat(64),
        created_at: 1_765_843_200,
        kind: 8,
        content: String::new(),
        tags: vec![
            vec!["a".into(), coordinate.into()],
            vec!["p".into(), pubkey.into()],
            vec!["period".into(), period_key.into()],
        ],
        sig: "e".repeat(128),
    }
}

async fn seed_prepared_run(repo: &FakeRepo, status: AwardRunStatus) -> SignedNostrEvent {
    let coordinate = "30009:issuerpubkey:diviner-of-the-day";
    let event = signed_event("stored-event", coordinate, FIRST, "2026-04-14");
    let mut run = run_with_winner(FIRST, "stored winner");
    run.status = status;
    run.prepared_award_event = Some(serde_json::to_string(&event).unwrap());
    run.award_event_id = Some(event.id.clone());
    repo.upsert_award_run(run).await.unwrap();
    repo.insert_badge_definition_seed(&BadgeDefinitionRecord::published(
        &award_for_period_kind("day").unwrap(),
        "https://cdn.divine.video/logo.png",
        "definition-id",
        coordinate,
    ))
    .await
    .unwrap();
    event
}

async fn execute(
    now: DateTime<Utc>,
    repo: &FakeRepo,
    candidates: &FakeCandidates,
    publisher: &FakePublisher,
    discord: &FakeDiscord,
) -> Result<TickOutcome, AppError> {
    let campaigns = FakeCampaignClient::default();
    execute_with_claim_time(
        now,
        now,
        &config(),
        &campaigns,
        repo,
        candidates,
        publisher,
        discord,
    )
    .await
}

#[derive(Clone, Copy)]
struct FakeClock(DateTime<Utc>);

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

async fn execute_with_claim_time(
    tick_started_at: DateTime<Utc>,
    claim_time: DateTime<Utc>,
    config: &AppConfig,
    campaigns: &FakeCampaignClient,
    repo: &FakeRepo,
    candidates: &FakeCandidates,
    publisher: &FakePublisher,
    discord: &FakeDiscord,
) -> Result<TickOutcome, AppError> {
    run_award_tick_with_clock(
        tick_started_at,
        &FakeClock(claim_time),
        config,
        repo,
        candidates,
        publisher,
        discord,
        campaigns,
    )
    .await
}

fn tick() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap()
}

#[test]
fn passes_exact_closed_boundaries_and_window_to_ranked_candidates() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 18, 45, 0).unwrap(),
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(
            candidates.calls.borrow().as_slice(),
            &[(
                Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap(),
                period_end(),
                CANDIDATE_WINDOW_WITH_EXCLUSIONS,
            )]
        );
    });
}

const CANDIDATE_WINDOW_WITH_EXCLUSIONS: usize = 20;

#[test]
fn excluded_personnel_do_not_consume_the_eligible_candidate_window() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut ranked = DIVINER_AWARD_EXCLUDED_PUBKEYS
            .iter()
            .enumerate()
            .map(|(index, pubkey)| candidate(pubkey, "excluded personnel", index as u64 + 1))
            .collect::<Vec<_>>();
        ranked.extend((0..10).map(|index| {
            let pubkey = format!("{index:064x}");
            candidate(&pubkey, "eligible creator", index + 11)
        }));
        let expected_winner = ranked[DIVINER_AWARD_EXCLUDED_PUBKEYS.len()].pubkey.clone();
        let candidates = FakeCandidates {
            candidates: ranked,
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(
            candidates.calls.borrow()[0].2,
            CANDIDATE_WINDOW_WITH_EXCLUSIONS
        );
        assert_eq!(
            outcome.runs[0].winner_pubkey.as_deref(),
            Some(expected_winner.as_str())
        );
    });
}

#[test]
fn preserves_upstream_order_without_recomputing_scores() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut first = candidate(FIRST, "first upstream", 1);
        first.score = 1.0;
        first.views = 1;
        let mut second = candidate(SECOND, "higher metrics", 2);
        second.score = 100.0;
        second.views = 10_000;
        let candidates = FakeCandidates {
            candidates: vec![first, second],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
        assert_eq!(publisher.published_awards.borrow()[0].tags[1][1], FIRST);
    });
}

#[test]
fn founder_and_invalid_activity_proofs_are_skipped_for_the_next_candidate() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut post_end = candidate(FIRST, "post end", 2);
        post_end.latest_eligible_publication_at = period_end();
        let candidates = FakeCandidates {
            candidates: vec![
                candidate(FOUNDER, "founder", 1),
                post_end,
                candidate(SECOND, "active", 3),
            ],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(SECOND));
    });
}

#[test]
fn post_end_activity_cannot_make_a_creator_eligible_on_a_historical_tick() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut post_end = candidate(FIRST, "post end", 1);
        post_end.latest_eligible_publication_at = period_end() + Duration::hours(12);
        let candidates = FakeCandidates {
            candidates: vec![post_end],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 23, 55, 0).unwrap(),
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
        assert_eq!(*publisher.count.borrow(), 0);
    });
}

#[test]
fn candidate_fetch_failure_marks_failed_fetch_without_publication() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            failure: Some(CandidateFailure::Api("ranking unavailable".into())),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::FailedFetch);
        assert_eq!(*publisher.count.borrow(), 0);
    });
}

#[test]
fn transport_empty_candidates_marks_skipped_inactive() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            failure: Some(CandidateFailure::Empty),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
        assert_eq!(*publisher.count.borrow(), 0);
    });
}

#[test]
fn duplicate_completed_tick_does_not_refetch_or_republish() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();
        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(candidates.calls.borrow().len(), 1);
        assert_eq!(*publisher.count.borrow(), 2);
        assert_eq!(publisher.published_awards.borrow().len(), 1);
        assert_eq!(discord.messages.borrow().len(), 1);
    });
}

#[test]
fn awarded_states_retry_only_discord_from_the_stored_receipt() {
    block_on(async {
        for status in [
            AwardRunStatus::Awarded,
            AwardRunStatus::AwardedDiscordPending,
        ] {
            let repo = FakeRepo::default();
            let mut run = run_with_winner(FIRST, "stored winner");
            run.status = status;
            run.award_event_id = Some("award-event-id".into());
            repo.upsert_award_run(run).await.unwrap();
            let candidates = FakeCandidates::default();
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
                .await
                .unwrap();

            assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
            assert!(candidates.calls.borrow().is_empty());
            assert_eq!(*publisher.count.borrow(), 0);
            assert_eq!(discord.messages.borrow().len(), 1);
            assert_eq!(
                discord.messages.borrow()[0],
                format!(
                    "Diviner of the Day: stored winner — 7 positive reactors, 5 commenters, 3 reposts, and 20 unique viewers.\nhttps://divine.video/{}",
                    divine_badges::nip19::encode_npub(FIRST).unwrap()
                )
            );
        }
    });
}

#[test]
fn discord_retry_sanitizes_persisted_profile_fields_without_refetching() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "Åda\nhttps://evil.example/profile\t東京");
        run.winner_nip05 = Some("evil.example#@divine.video".into());
        run.status = AwardRunStatus::AwardedDiscordPending;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert!(candidates.calls.borrow().is_empty());
        assert_eq!(*publisher.count.borrow(), 0);
        assert_eq!(
            discord.messages.borrow()[0],
            format!(
                "Diviner of the Day: Åda 東京 — 7 positive reactors, 5 commenters, 3 reposts, and 20 unique viewers.\nhttps://divine.video/{}",
                divine_badges::nip19::encode_npub(FIRST).unwrap()
            )
        );
    });
}

#[test]
fn discord_retry_with_an_oversized_unicode_name_posts_a_bounded_complete_announcement() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, &"Å🚀東".repeat(3_000));
        run.status = AwardRunStatus::AwardedDiscordPending;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        let message = &discord.messages.borrow()[0];
        let expected_link = format!(
            "https://divine.video/{}",
            divine_badges::nip19::encode_npub(FIRST).unwrap()
        );
        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert!(message.encode_utf16().count() <= 2_000);
        assert_eq!(message.lines().last(), Some(expected_link.as_str()));
        assert!(message
            .contains("— 7 positive reactors, 5 commenters, 3 reposts, and 20 unique viewers."));
        assert!(candidates.calls.borrow().is_empty());
        assert_eq!(*publisher.count.borrow(), 0);
    });
}

#[test]
fn legacy_discord_retry_with_an_incomplete_receipt_posts_a_fallback_and_completes() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "legacy winner");
        run.status = AwardRunStatus::AwardedDiscordPending;
        run.positive_reactors = None;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        let stored = repo
            .runs
            .borrow()
            .get(&("diviner_of_the_day".into(), "2026-04-14".into()))
            .cloned()
            .unwrap();
        assert_eq!(stored.status, AwardRunStatus::Completed);
        assert_eq!(stored.discord_claim_token, None);
        assert_eq!(stored.discord_lease_expires_at, None);
        assert_eq!(stored.error_message, None);
        assert!(candidates.calls.borrow().is_empty());
        assert_eq!(*publisher.count.borrow(), 0);
        assert_eq!(
            discord.messages.borrow()[0],
            format!(
                "Diviner of the Day: legacy winner earned this badge.\nhttps://divine.video/{}",
                divine_badges::nip19::encode_npub(FIRST).unwrap()
            )
        );
    });
}

#[test]
fn invalid_discord_receipt_stays_pending_without_aborting_the_tick() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "invalid receipt");
        run.status = AwardRunStatus::AwardedDiscordPending;
        run.positive_reactors = Some(-1);
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(
            outcome.runs[0].status,
            AwardRunStatus::AwardedDiscordPending
        );
        assert!(outcome.runs[0]
            .error_message
            .as_deref()
            .unwrap()
            .contains("invalid negative positive reactors"));
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn discord_retry_never_truncates_the_stored_pubkey() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "");
        run.winner_display_name = None;
        run.winner_name = None;
        run.status = AwardRunStatus::AwardedDiscordPending;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert!(discord.messages.borrow()[0].contains(FIRST));
    });
}

#[test]
fn all_zero_candidate_still_wins() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut zero = candidate(FIRST, "zero engagement", 1);
        zero.views = 0;
        zero.unique_viewers = 0;
        zero.loops = 0.0;
        zero.videos_with_views = 0;
        zero.positive_reactors = 0;
        zero.distinct_commenters = 0;
        zero.distinct_reposters = 0;
        zero.distinct_positive_engagers = 0;
        zero.engagement_tier = 0;
        zero.engagement_rate = 0.0;
        zero.score = 0.0;
        let candidates = FakeCandidates {
            candidates: vec![zero],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
    });
}

#[test]
fn empty_and_fully_ineligible_fake_results_are_skipped() {
    block_on(async {
        let mut too_old = candidate(FIRST, "too old", 1);
        too_old.latest_eligible_publication_at =
            period_end() - Duration::days(30) - Duration::seconds(1);
        for list in [
            Vec::new(),
            vec![candidate(FOUNDER, "founder", 1)],
            vec![too_old],
        ] {
            let repo = FakeRepo::default();
            let candidates = FakeCandidates {
                candidates: list,
                ..Default::default()
            };
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
                .await
                .unwrap();

            assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
            assert_eq!(*publisher.count.borrow(), 0);
        }
    });
}

#[test]
fn atomically_claims_the_first_winner_and_prepared_event_before_relay_send() {
    block_on(async {
        let repo = FakeRepo::default();
        *repo.winner_claim_override.borrow_mut() =
            Some(run_with_winner(SECOND, "canonical winner"));
        let canonical_event = signed_event(
            "canonical-event",
            "30009:issuerpubkey:diviner-of-the-day",
            SECOND,
            "2026-04-14",
        );
        *repo.prepared_claim_override.borrow_mut() = Some(canonical_event.clone());
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "losing concurrent writer", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(SECOND));
        assert_eq!(
            publisher.published_awards.borrow().as_slice(),
            &[canonical_event]
        );
        let operations = repo.operations.borrow();
        let winner_claim = operations
            .iter()
            .position(|value| value == "claim-winner")
            .unwrap();
        let prepared_claim = operations
            .iter()
            .position(|value| value == "claim-prepared")
            .unwrap();
        let relay_send = operations
            .iter()
            .position(|value| value == "publish-prepared")
            .unwrap();
        assert!(winner_claim < prepared_claim && prepared_claim < relay_send);
    });
}

#[test]
fn failed_definition_resumes_from_stored_winner_without_refetching() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::FailedDefinition;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates {
            candidates: vec![candidate(SECOND, "changed upstream winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert!(candidates.calls.borrow().is_empty());
        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
        assert_eq!(publisher.published_awards.borrow()[0].tags[1][1], FIRST);
    });
}

#[test]
fn failed_award_resumes_the_stored_prepared_event_without_refetch_or_resigning() {
    block_on(async {
        let repo = FakeRepo::default();
        let event = signed_event(
            "stored-event",
            "30009:issuerpubkey:diviner-of-the-day",
            FIRST,
            "2026-04-14",
        );
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::FailedAward;
        run.prepared_award_event = Some(serde_json::to_string(&event).unwrap());
        run.award_event_id = Some(event.id.clone());
        repo.upsert_award_run(run).await.unwrap();
        repo.insert_badge_definition_seed(&BadgeDefinitionRecord::published(
            &award_for_period_kind("day").unwrap(),
            "https://cdn.divine.video/logo.png",
            "definition-id",
            "30009:issuerpubkey:diviner-of-the-day",
        ))
        .await
        .unwrap();
        let candidates = FakeCandidates {
            candidates: vec![candidate(SECOND, "changed upstream winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert!(candidates.calls.borrow().is_empty());
        assert_eq!(*publisher.prepare_count.borrow(), 0);
        assert_eq!(publisher.published_awards.borrow().as_slice(), &[event]);
    });
}

#[test]
fn preparation_failure_records_failed_award_and_retries_from_the_stored_winner() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let mut publisher = FakePublisher::new(repo.operations.clone());
        publisher.fail_prepare = true;
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::FailedAward);
        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
        assert!(outcome.runs[0]
            .error_message
            .as_deref()
            .unwrap()
            .contains("signing unavailable"));
        assert!(publisher.published_awards.borrow().is_empty());
        assert_eq!(candidates.calls.borrow().len(), 1);

        let changed_upstream = FakeCandidates {
            candidates: vec![candidate(SECOND, "changed winner", 1)],
            ..Default::default()
        };
        let retry_publisher = FakePublisher::new(repo.operations.clone());
        let retried = execute(
            tick() + Duration::minutes(1),
            &repo,
            &changed_upstream,
            &retry_publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(retried.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(retried.runs[0].winner_pubkey.as_deref(), Some(FIRST));
        assert!(changed_upstream.calls.borrow().is_empty());
        assert_eq!(retry_publisher.published_awards.borrow().len(), 1);
    });
}

#[test]
fn stale_signing_failure_cannot_overwrite_a_concurrently_prepared_award() {
    block_on(async {
        let repo = FakeRepo::default();
        *repo.prepare_before_mark_preparation_failed.borrow_mut() = true;
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let mut publisher = FakePublisher::new(repo.operations.clone());
        publisher.fail_prepare = true;
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::AwardPrepared);
        assert_eq!(
            outcome.runs[0].award_event_id.as_deref(),
            Some("concurrent-event-id")
        );
        assert!(publisher.published_awards.borrow().is_empty());
    });
}

#[test]
fn late_definition_failure_cannot_overwrite_a_preparation_failure() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::FailedAward;
        run.error_message = Some("signing unavailable".into());
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let mut publisher = FakePublisher::new(repo.operations.clone());
        publisher.fail_definition = true;
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::FailedAward);
        assert_eq!(
            outcome.runs[0].error_message.as_deref(),
            Some("signing unavailable")
        );
    });
}

#[test]
fn mismatched_stored_prepared_payloads_are_never_published() {
    block_on(async {
        let coordinate = "30009:issuerpubkey:diviner-of-the-day";
        let canonical = signed_event("stored-event", coordinate, FIRST, "2026-04-14");
        let mut wrong_content = canonical.clone();
        wrong_content.content = "tampered".into();
        let mut wrong_coordinate = canonical.clone();
        wrong_coordinate.tags[0][1] = "30009:other:badge".into();
        let mut wrong_winner = canonical.clone();
        wrong_winner.tags[1][1] = SECOND.into();
        let mut wrong_period = canonical.clone();
        wrong_period.tags[2][1] = "2026-04-13".into();

        for event in [wrong_content, wrong_coordinate, wrong_winner, wrong_period] {
            let repo = FakeRepo::default();
            let mut run = run_with_winner(FIRST, "stored winner");
            run.status = AwardRunStatus::FailedAward;
            run.prepared_award_event = Some(serde_json::to_string(&event).unwrap());
            run.award_event_id = Some(event.id.clone());
            repo.upsert_award_run(run).await.unwrap();
            repo.insert_badge_definition_seed(&BadgeDefinitionRecord::published(
                &award_for_period_kind("day").unwrap(),
                "https://cdn.divine.video/logo.png",
                "definition-id",
                coordinate,
            ))
            .await
            .unwrap();
            let candidates = FakeCandidates::default();
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
                .await
                .unwrap();

            assert_eq!(outcome.runs[0].status, AwardRunStatus::FailedAward);
            assert!(publisher.published_awards.borrow().is_empty());
            assert!(candidates.calls.borrow().is_empty());
        }
    });
}

#[test]
fn relay_acceptance_followed_by_repository_failure_retries_the_identical_event() {
    block_on(async {
        let repo = FakeRepo::default();
        *repo.fail_mark_awarded_once.borrow_mut() = true;
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let first_error = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap_err();
        assert!(first_error.to_string().contains("simulated crash"));

        let changed_upstream = FakeCandidates {
            candidates: vec![candidate(SECOND, "changed winner", 1)],
            ..Default::default()
        };
        let outcome = execute(tick(), &repo, &changed_upstream, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert!(changed_upstream.calls.borrow().is_empty());
        assert_eq!(*publisher.prepare_count.borrow(), 1);
        let published = publisher.published_awards.borrow();
        assert_eq!(published.len(), 2);
        assert_eq!(published[0], published[1]);
        assert_eq!(published[0].id, published[1].id);
    });
}

#[test]
fn stale_relay_success_cannot_regress_or_reannounce_a_completed_run() {
    block_on(async {
        let repo = FakeRepo::default();
        seed_prepared_run(&repo, AwardRunStatus::AwardPrepared).await;
        *repo.complete_before_mark_awarded.borrow_mut() = true;
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn stale_relay_failure_cannot_regress_a_completed_run() {
    block_on(async {
        let repo = FakeRepo::default();
        seed_prepared_run(&repo, AwardRunStatus::AwardPrepared).await;
        *repo.complete_before_mark_award_failed.borrow_mut() = true;
        let candidates = FakeCandidates::default();
        let mut publisher = FakePublisher::new(repo.operations.clone());
        publisher.fail_award = true;
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn worker_with_an_unexpired_discord_lease_does_not_publish_or_post() {
    block_on(async {
        let repo = FakeRepo::default();
        seed_prepared_run(&repo, AwardRunStatus::DiscordSending).await;
        {
            let mut runs = repo.runs.borrow_mut();
            let run = runs
                .get_mut(&("diviner_of_the_day".into(), "2026-04-14".into()))
                .unwrap();
            run.discord_claim_token = Some("other-worker".into());
            run.discord_lease_expires_at = Some(tick() + Duration::minutes(5));
        }
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::DiscordSending);
        assert!(publisher.published_awards.borrow().is_empty());
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn delayed_pipeline_claims_discord_from_fresh_claim_time() {
    block_on(async {
        let repo = FakeRepo::default();
        seed_prepared_run(&repo, AwardRunStatus::Awarded).await;
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let claim_time = tick() + Duration::minutes(12);

        let outcome = execute_with_claim_time(
            tick(),
            claim_time,
            &config(),
            &FakeCampaignClient::default(),
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(
            repo.discord_claim_times.borrow().as_slice(),
            &[(claim_time, claim_time + Duration::minutes(5))]
        );
    });
}

#[test]
fn webhook_timeout_finishes_before_the_discord_lease_can_be_reclaimed() {
    block_on(async {
        let repo = FakeRepo::default();
        seed_prepared_run(&repo, AwardRunStatus::Awarded).await;
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let claim_time = tick() + Duration::minutes(12);
        let discord = InFlightReclaimDiscord {
            repo: &repo,
            claim_time,
            timeout: RefCell::new(None),
            reclaim_acquired: RefCell::new(None),
        };

        let outcome = run_award_tick_with_clock(
            tick(),
            &FakeClock(claim_time),
            &config(),
            &repo,
            &candidates,
            &publisher,
            &discord,
            &FakeCampaignClient::default(),
        )
        .await
        .unwrap();

        let timeout = discord.timeout.borrow().unwrap();
        let (_, lease_expires_at) = repo.discord_claim_times.borrow()[0];
        let lease_duration = (lease_expires_at - claim_time).to_std().unwrap();
        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(timeout, std::time::Duration::from_secs(20));
        assert!(timeout < lease_duration);
        assert_eq!(
            lease_duration - timeout,
            std::time::Duration::from_secs(4 * 60 + 40)
        );
        assert_eq!(*discord.reclaim_acquired.borrow(), Some(false));
    });
}

#[test]
fn discord_claim_allows_one_live_worker_and_reclaims_after_expiry() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::Awarded;
        repo.upsert_award_run(run).await.unwrap();
        let now = tick();

        let first = repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "worker-a",
                now,
                now + Duration::minutes(5),
            )
            .await
            .unwrap();
        let overlapping = repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "worker-b",
                now + Duration::minutes(1),
                now + Duration::minutes(6),
            )
            .await
            .unwrap();
        let reclaimed = repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "worker-b",
                now + Duration::minutes(5),
                now + Duration::minutes(10),
            )
            .await
            .unwrap();

        assert!(first.acquired);
        assert!(!overlapping.acquired);
        assert!(reclaimed.acquired);
        assert_eq!(
            reclaimed.run.discord_claim_token.as_deref(),
            Some("worker-b")
        );
    });
}

#[test]
fn two_workers_reaching_discord_concurrently_allow_only_the_claimant_to_post() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::Awarded;
        repo.upsert_award_run(run).await.unwrap();
        let now = tick();
        let discord = FakeDiscord::default();

        let worker_a = repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "worker-a",
                now,
                now + Duration::minutes(5),
            )
            .await
            .unwrap();
        let worker_b = repo
            .claim_discord_delivery(
                "diviner_of_the_day",
                "2026-04-14",
                "worker-b",
                now,
                now + Duration::minutes(5),
            )
            .await
            .unwrap();

        for claim in [worker_a, worker_b] {
            if claim.acquired {
                discord
                    .post_message("one announcement", std::time::Duration::from_secs(1))
                    .await
                    .unwrap();
            }
        }

        assert_eq!(discord.messages.borrow().as_slice(), &["one announcement"]);
    });
}

#[test]
fn discord_failure_releases_the_lease_and_remains_retryable() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut run = run_with_winner(FIRST, "stored winner");
        run.status = AwardRunStatus::Awarded;
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        *discord.failure.borrow_mut() = Some("webhook unavailable".into());

        let failed = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(failed.runs[0].status, AwardRunStatus::AwardedDiscordPending);
        assert_eq!(failed.runs[0].discord_claim_token, None);
        assert_eq!(failed.runs[0].discord_lease_expires_at, None);

        *discord.failure.borrow_mut() = None;
        let retried = execute(
            tick() + Duration::minutes(1),
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .unwrap();
        assert_eq!(retried.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(discord.messages.borrow().len(), 2);
    });
}

#[test]
fn persists_the_complete_winner_receipt_before_preparing_or_publishing() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut winner = candidate(FIRST, "receipt winner", 1);
        winner.name = "winner name".into();
        winner.nip05 = Some("winner@divine.video".into());
        winner.picture = "https://cdn.divine.video/winner.png".into();
        let candidates = FakeCandidates {
            candidates: vec![winner],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        let run = &outcome.runs[0];
        assert_eq!(run.winner_pubkey.as_deref(), Some(FIRST));
        assert_eq!(run.winner_display_name.as_deref(), Some("receipt winner"));
        assert_eq!(run.winner_name.as_deref(), Some("winner name"));
        assert_eq!(run.winner_nip05.as_deref(), Some("winner@divine.video"));
        assert_eq!(
            run.winner_picture.as_deref(),
            Some("https://cdn.divine.video/winner.png")
        );
        assert_eq!(
            run.latest_eligible_publication_at,
            Some(period_end() - Duration::hours(12))
        );
        assert_eq!(run.loops, Some(1.5));
        assert_eq!(run.views, Some(30));
        assert_eq!(run.unique_viewers, Some(20));
        assert_eq!(run.videos_with_views, Some(2));
        assert_eq!(run.positive_reactors, Some(7));
        assert_eq!(run.distinct_commenters, Some(5));
        assert_eq!(run.distinct_reposters, Some(3));
        assert_eq!(run.distinct_positive_engagers, Some(10));
        assert_eq!(run.engagement_tier, Some(1));
        assert_eq!(run.engagement_rate, Some(0.5));
        assert_eq!(run.score, Some(88.25));
        assert!(run.prepared_award_event.is_some());
        let operations = repo.operations.borrow();
        let receipt = operations
            .iter()
            .position(|value| value == "claim-winner")
            .unwrap();
        let prepared = operations
            .iter()
            .position(|value| value == "claim-prepared")
            .unwrap();
        assert!(receipt < prepared);
    });
}

#[test]
fn captured_response_persists_and_reuses_the_engaged_winner_receipt_on_discord_retry() {
    block_on(async {
        const ENGAGED: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        const ZERO_ENGAGEMENT: &str =
            "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
        let body = format!(
            r#"{{
              "start": "2026-04-14T00:00:00Z",
              "end": "2026-04-15T00:00:00Z",
              "entries": [
                {{
                  "pubkey": "{ENGAGED}",
                  "name": "ada",
                  "display_name": "Ada",
                  "nip05": "ada@divine.video",
                  "picture": "https://cdn.divine.video/ada.png",
                  "latest_eligible_publication_at": "2026-04-14T12:00:00Z",
                  "views": 120,
                  "unique_viewers": 80,
                  "loops": 1.5,
                  "videos_with_views": 2,
                  "positive_reactors": 42,
                  "distinct_commenters": 9,
                  "distinct_reposters": 7,
                  "distinct_positive_engagers": 50,
                  "engagement_tier": 1,
                  "engagement_rate": 0.625,
                  "score": 91.25,
                  "rank": 1
                }},
                {{
                  "pubkey": "{ZERO_ENGAGEMENT}",
                  "name": "reach-only",
                  "display_name": "Reach Only",
                  "nip05": null,
                  "picture": "",
                  "latest_eligible_publication_at": "2026-04-14T10:00:00Z",
                  "views": 12000,
                  "unique_viewers": 9000,
                  "loops": 1.75,
                  "videos_with_views": 8,
                  "positive_reactors": 0,
                  "distinct_commenters": 0,
                  "distinct_reposters": 0,
                  "distinct_positive_engagers": 0,
                  "engagement_tier": 0,
                  "engagement_rate": 0.0,
                  "score": 19.75,
                  "rank": 2
                }}
              ]
            }}"#
        );
        let start = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap();
        let ranked = ranked_candidates_for_period(
            |_| Ok((200, body)),
            "https://api.divine.video",
            start,
            period_end(),
            10,
        )
        .await
        .unwrap();

        assert_eq!(ranked[0].pubkey, ENGAGED);
        assert_eq!(ranked[0].engagement_tier, 1);
        assert!(ranked[0].views < ranked[1].views);
        assert_eq!(ranked[1].engagement_tier, 0);

        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: ranked,
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        *discord.failure.borrow_mut() = Some("webhook unavailable".into());

        let first_attempt = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();
        let stored = &first_attempt.runs[0];
        assert_eq!(stored.status, AwardRunStatus::AwardedDiscordPending);
        assert_eq!(stored.winner_pubkey.as_deref(), Some(ENGAGED));
        assert_eq!(stored.positive_reactors, Some(42));
        assert_eq!(stored.distinct_commenters, Some(9));
        assert_eq!(stored.distinct_reposters, Some(7));
        assert_eq!(stored.distinct_positive_engagers, Some(50));
        assert_eq!(stored.engagement_tier, Some(1));
        assert_eq!(stored.engagement_rate, Some(0.625));
        assert_eq!(stored.score, Some(91.25));
        assert_eq!(publisher.published_awards.borrow()[0].tags[1][1], ENGAGED);

        *discord.failure.borrow_mut() = None;
        let retried = execute(
            tick() + Duration::minutes(1),
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        let expected_message = "Diviner of the Day: Ada — 42 positive reactors, 9 commenters, 7 reposts, and 80 unique viewers.\nhttps://ada.divine.video".to_string();
        assert_eq!(retried.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(candidates.calls.borrow().len(), 1);
        assert_eq!(
            discord.messages.borrow().as_slice(),
            &[expected_message.clone(), expected_message]
        );
    });
}

#[test]
fn accepts_js_safe_boundary_and_preserves_full_pubkey_fallback() {
    block_on(async {
        let repo = FakeRepo::default();
        let mut winner = candidate(FIRST, "", 1);
        winner.name.clear();
        winner.views = JS_SAFE_MAX;
        winner.unique_viewers = JS_SAFE_MAX;
        winner.videos_with_views = JS_SAFE_MAX;
        winner.positive_reactors = JS_SAFE_MAX;
        winner.distinct_commenters = JS_SAFE_MAX;
        winner.distinct_reposters = JS_SAFE_MAX;
        winner.distinct_positive_engagers = JS_SAFE_MAX;
        let candidates = FakeCandidates {
            candidates: vec![winner],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
            .await
            .unwrap();

        let run = &outcome.runs[0];
        assert_eq!(run.winner_display_name.as_deref(), Some(FIRST));
        for value in [
            run.views,
            run.unique_viewers,
            run.videos_with_views,
            run.positive_reactors,
            run.distinct_commenters,
            run.distinct_reposters,
            run.distinct_positive_engagers,
        ] {
            assert_eq!(value, Some(JS_SAFE_MAX as i64));
        }
    });
}

#[test]
fn rejects_every_counter_above_js_safe_boundary_without_publication() {
    block_on(async {
        for field in [
            "views",
            "unique_viewers",
            "videos_with_views",
            "positive_reactors",
            "distinct_commenters",
            "distinct_reposters",
            "distinct_positive_engagers",
        ] {
            let repo = FakeRepo::default();
            let mut winner = candidate(FIRST, "overflow", 1);
            match field {
                "views" => winner.views = JS_SAFE_MAX + 1,
                "unique_viewers" => winner.unique_viewers = JS_SAFE_MAX + 1,
                "videos_with_views" => winner.videos_with_views = JS_SAFE_MAX + 1,
                "positive_reactors" => winner.positive_reactors = JS_SAFE_MAX + 1,
                "distinct_commenters" => winner.distinct_commenters = JS_SAFE_MAX + 1,
                "distinct_reposters" => winner.distinct_reposters = JS_SAFE_MAX + 1,
                "distinct_positive_engagers" => winner.distinct_positive_engagers = JS_SAFE_MAX + 1,
                _ => unreachable!(),
            }
            let candidates = FakeCandidates {
                candidates: vec![winner],
                ..Default::default()
            };
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(tick(), &repo, &candidates, &publisher, &discord)
                .await
                .unwrap();

            assert_eq!(
                outcome.runs[0].status,
                AwardRunStatus::FailedFetch,
                "{field}"
            );
            assert!(outcome.runs[0]
                .error_message
                .as_deref()
                .unwrap()
                .contains(field));
            assert_eq!(*publisher.count.borrow(), 0, "{field}");
        }
    });
}

#[test]
fn completed_award_creates_both_campaigns_once() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let campaigns = FakeCampaignClient::default();
        let config = config_with_engagement();

        execute_with_claim_time(
            tick(),
            tick(),
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("first tick");
        execute_with_claim_time(
            tick(),
            tick(),
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("second tick");

        // Review Focus 3 downstream: the claim, not the remote API, is what makes
        // this once-only from our side.
        let created = campaigns.created.borrow();
        assert_eq!(created.len(), 2);
        assert!(created
            .iter()
            .any(|campaign| campaign.automation_key.ends_with("-winner")));
        assert!(created
            .iter()
            .any(|campaign| campaign.automation_key.ends_with("-broadcast")));
    });
}

#[test]
fn an_engagement_outage_does_not_fail_the_award_tick() {
    // Review Focus 4.
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let campaigns = FakeCampaignClient::failing();
        let config = config_with_engagement();

        let outcome = execute_with_claim_time(
            tick(),
            tick(),
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await;

        assert!(
            outcome.is_ok(),
            "the award must complete even if notification fails"
        );
        assert_eq!(outcome.unwrap().runs[0].status, AwardRunStatus::Completed);
    });
}

#[test]
fn a_failed_notification_is_not_recorded_as_sent_and_a_later_tick_retries_it() {
    // Review Focus 4: the notification must not be recorded as sent.
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let campaigns = FakeCampaignClient::failing();
        let config = config_with_engagement();

        execute_with_claim_time(
            tick(),
            tick(),
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("tick during outage");
        assert!(campaigns.created.borrow().is_empty());
        assert!(repo.push_notified.borrow().is_empty());

        *campaigns.failure.borrow_mut() = false;
        let later = tick() + chrono::Duration::hours(1);
        execute_with_claim_time(
            later,
            later,
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("tick after recovery");

        assert_eq!(campaigns.created.borrow().len(), 2);
        assert_eq!(repo.push_notified.borrow().len(), 1);
    });
}

#[test]
fn no_campaigns_without_engagement_access_credentials() {
    // Unconfigured means closed: a URL without credentials cannot authenticate.
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let campaigns = FakeCampaignClient::default();
        let config = AppConfig {
            engagement_access_client_secret: None,
            ..config_with_engagement()
        };

        execute_with_claim_time(
            tick(),
            tick(),
            &config,
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("tick");

        assert!(campaigns.created.borrow().is_empty());
        assert!(repo.push_notified.borrow().is_empty());
    });
}

#[test]
fn no_campaigns_without_a_configured_engagement_url() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let campaigns = FakeCampaignClient::default();

        execute_with_claim_time(
            tick(),
            tick(),
            &config(),
            &campaigns,
            &repo,
            &candidates,
            &publisher,
            &discord,
        )
        .await
        .expect("tick");

        assert!(campaigns.created.borrow().is_empty());
    });
}
