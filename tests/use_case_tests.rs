use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use divine_badges::awards::award_for_period_kind;
use divine_badges::config::AppConfig;
use divine_badges::error::AppError;
use divine_badges::models::{
    AwardRun, BadgeDefinitionRecord, CreatorLatestVideo, DivinerCandidate,
};
use divine_badges::ports::{
    AwardRepository, BadgePublisher, CreatorActivityClient, DiscordClient, DivinerCandidatesClient,
};
use divine_badges::state::AwardRunStatus;
use divine_badges::use_cases::{run_award_tick, TickOutcome};
use futures::executor::block_on;

const FOUNDER: &str = "d95aa8fc0eff8e488952495b8064991d27fb96ed8652f12cdedc5a4e8b5ae540";
const FIRST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SECOND: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const JS_SAFE_MAX: u64 = 9_007_199_254_740_991;
type CandidateCall = (DateTime<Utc>, DateTime<Utc>, usize);
type ActivityCall = (String, DateTime<Utc>);

struct FakeRepo {
    definitions: RefCell<HashMap<String, BadgeDefinitionRecord>>,
    runs: RefCell<HashMap<(String, String), AwardRun>>,
    operations: Rc<RefCell<Vec<String>>>,
}

impl Default for FakeRepo {
    fn default() -> Self {
        Self {
            definitions: RefCell::new(HashMap::new()),
            runs: RefCell::new(HashMap::new()),
            operations: Rc::new(RefCell::new(Vec::new())),
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

    async fn save_award_run(&self, run: &AwardRun) -> Result<AwardRun, AppError> {
        self.operations.borrow_mut().push("save-award-run".into());
        self.runs.borrow_mut().insert(
            (run.award_slug.clone(), run.period_key.clone()),
            run.clone(),
        );
        Ok(run.clone())
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
        self.update(slug, key, AwardRunStatus::FailedFetch, Some(error))
    }

    async fn mark_definition_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        self.update(slug, key, AwardRunStatus::FailedDefinition, Some(error))
    }

    async fn mark_award_failed(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        self.update(slug, key, AwardRunStatus::FailedAward, Some(error))
    }

    async fn mark_awarded(
        &self,
        slug: &str,
        key: &str,
        event_id: &str,
    ) -> Result<AwardRun, AppError> {
        let mut run = self.update(slug, key, AwardRunStatus::Awarded, None)?;
        run.award_event_id = Some(event_id.into());
        self.runs
            .borrow_mut()
            .insert((slug.into(), key.into()), run.clone());
        Ok(run)
    }

    async fn mark_discord_pending(
        &self,
        slug: &str,
        key: &str,
        error: &str,
    ) -> Result<AwardRun, AppError> {
        self.update(
            slug,
            key,
            AwardRunStatus::AwardedDiscordPending,
            Some(error),
        )
    }

    async fn mark_completed(&self, slug: &str, key: &str) -> Result<AwardRun, AppError> {
        let mut run = self.update(slug, key, AwardRunStatus::Completed, None)?;
        run.discord_message_sent = true;
        self.runs
            .borrow_mut()
            .insert((slug.into(), key.into()), run.clone());
        Ok(run)
    }

    async fn mark_skipped_inactive(&self, slug: &str, key: &str) -> Result<AwardRun, AppError> {
        self.update(slug, key, AwardRunStatus::SkippedInactive, None)
    }
}

#[derive(Default)]
struct FakeCandidates {
    candidates: Vec<DivinerCandidate>,
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
        Ok(self.candidates.clone())
    }
}

enum ActivityResult {
    Found(CreatorLatestVideo),
    Failed(String),
}

#[derive(Default)]
struct FakeActivity {
    latest: HashMap<String, ActivityResult>,
    calls: RefCell<Vec<ActivityCall>>,
}

#[async_trait(?Send)]
impl CreatorActivityClient for FakeActivity {
    async fn latest_video_before(
        &self,
        pubkey: &str,
        period_end: DateTime<Utc>,
    ) -> Result<Option<CreatorLatestVideo>, AppError> {
        self.calls.borrow_mut().push((pubkey.into(), period_end));
        match self.latest.get(pubkey) {
            Some(ActivityResult::Found(video)) => Ok(Some(video.clone())),
            Some(ActivityResult::Failed(error)) => Err(AppError::Api(error.clone())),
            None => Ok(None),
        }
    }
}

struct FakePublisher {
    count: RefCell<usize>,
    winners: RefCell<Vec<String>>,
    operations: Rc<RefCell<Vec<String>>>,
}

impl FakePublisher {
    fn new(operations: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            count: RefCell::new(0),
            winners: RefCell::new(Vec::new()),
            operations,
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
    ) -> Result<divine_badges::nostr::DefinitionPublishResult, AppError> {
        *self.count.borrow_mut() += 1;
        self.operations
            .borrow_mut()
            .push("publish-definition".into());
        Ok(divine_badges::nostr::DefinitionPublishResult {
            definition_event_id: format!("definition-{}", award.slug),
            definition_coordinate: format!("30009:issuerpubkey:{}", award.d_tag),
        })
    }

    async fn publish_award(
        &self,
        _coordinate: &str,
        pubkey: &str,
        _period_key: &str,
    ) -> Result<String, AppError> {
        *self.count.borrow_mut() += 1;
        self.winners.borrow_mut().push(pubkey.into());
        self.operations.borrow_mut().push("publish-award".into());
        Ok("award-event-id".into())
    }
}

#[derive(Default)]
struct FakeDiscord {
    messages: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl DiscordClient for FakeDiscord {
    async fn post_message(&self, message: &str) -> Result<(), AppError> {
        self.messages.borrow_mut().push(message.into());
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
    }
}

fn candidate(pubkey: &str, display_name: &str, rank: u64) -> DivinerCandidate {
    DivinerCandidate {
        pubkey: pubkey.into(),
        name: display_name.into(),
        display_name: display_name.into(),
        nip05: None,
        picture: "https://cdn.divine.video/creator.png".into(),
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

fn video(year: i32, month: u32, day: u32, hour: u32) -> ActivityResult {
    ActivityResult::Found(CreatorLatestVideo {
        published_at: Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap(),
    })
}

async fn execute(
    now: DateTime<Utc>,
    repo: &FakeRepo,
    candidates: &FakeCandidates,
    activity: &FakeActivity,
    publisher: &FakePublisher,
    discord: &FakeDiscord,
) -> Result<TickOutcome, AppError> {
    run_award_tick(
        now,
        &config(),
        repo,
        candidates,
        activity,
        publisher,
        discord,
    )
    .await
}

#[test]
fn passes_exact_closed_boundaries_and_window_to_ranked_candidates() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates::default();
        let activity = FakeActivity::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 18, 45, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(
            candidates.calls.borrow().as_slice(),
            &[(
                Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap(),
                10,
            )]
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
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
        assert_eq!(
            activity.calls.borrow().as_slice(),
            &[(
                FIRST.into(),
                Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()
            )]
        );
        assert_eq!(publisher.winners.borrow().as_slice(), &[FIRST]);
    });
}

#[test]
fn skips_founder_and_inactive_candidate_then_awards_next_active_candidate() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![
                candidate(FOUNDER, "founder", 1),
                candidate(FIRST, "inactive", 2),
                candidate(SECOND, "active", 3),
            ],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([
                (FIRST.into(), video(2026, 1, 1, 0)),
                (SECOND.into(), video(2026, 4, 14, 12)),
            ]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(SECOND));
        assert_eq!(
            activity.calls.borrow().as_slice(),
            &[
                (
                    FIRST.into(),
                    Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()
                ),
                (
                    SECOND.into(),
                    Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()
                )
            ]
        );
    });
}

#[test]
fn anchors_activity_to_period_end_instead_of_retry_tick() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "boundary creator", 1)],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 3, 16, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 23, 55, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(
            activity.calls.borrow().as_slice(),
            &[(
                FIRST.into(),
                Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()
            )]
        );
    });
}

#[test]
fn post_period_video_cannot_make_a_creator_eligible_on_a_historical_retry_tick() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "post-period creator", 1)],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 15, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 23, 55, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
        assert_eq!(*publisher.count.borrow(), 0);
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn activity_failure_marks_failed_fetch_and_publishes_nothing() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "creator", 1)],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([(
                FIRST.into(),
                ActivityResult::Failed("activity unavailable".into()),
            )]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::FailedFetch);
        assert_eq!(*publisher.count.borrow(), 0);
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn duplicate_tick_returns_completed_before_refetch_or_republication() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![candidate(FIRST, "winner", 1)],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();
        let tick = Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap();

        execute(tick, &repo, &candidates, &activity, &publisher, &discord)
            .await
            .unwrap();
        let outcome = execute(tick, &repo, &candidates, &activity, &publisher, &discord)
            .await
            .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(candidates.calls.borrow().len(), 1);
        assert_eq!(*publisher.count.borrow(), 2);
        assert_eq!(discord.messages.borrow().len(), 1);
    });
}

#[test]
fn awarded_states_retry_discord_from_stored_receipt_only() {
    block_on(async {
        for status in [
            AwardRunStatus::Awarded,
            AwardRunStatus::AwardedDiscordPending,
        ] {
            let repo = FakeRepo::default();
            let award = award_for_period_kind("day").unwrap();
            let mut run = AwardRun::pending(award.slug, "2026-04-14", "day");
            run.status = status;
            run.winner_pubkey = Some(FIRST.into());
            run.winner_display_name = Some("stored winner".into());
            run.winner_nip05 = Some("winner@divine.video".into());
            run.loops = Some(1.5);
            run.views = Some(30);
            run.unique_viewers = Some(20);
            run.videos_with_views = Some(2);
            run.positive_reactors = Some(7);
            run.distinct_commenters = Some(5);
            run.distinct_reposters = Some(3);
            run.distinct_positive_engagers = Some(10);
            run.engagement_tier = Some(1);
            run.engagement_rate = Some(0.5);
            run.score = Some(88.25);
            run.award_event_id = Some("award-event-id".into());
            repo.upsert_award_run(run).await.unwrap();
            let candidates = FakeCandidates::default();
            let activity = FakeActivity::default();
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(
                Utc.with_ymd_and_hms(2026, 4, 15, 1, 5, 0).unwrap(),
                &repo,
                &candidates,
                &activity,
                &publisher,
                &discord,
            )
            .await
            .unwrap();

            assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
            assert!(candidates.calls.borrow().is_empty());
            assert!(activity.calls.borrow().is_empty());
            assert_eq!(*publisher.count.borrow(), 0);
            assert_eq!(discord.messages.borrow().len(), 1);
        }
    });
}

#[test]
fn discord_retry_falls_back_to_the_complete_stored_pubkey() {
    block_on(async {
        let repo = FakeRepo::default();
        let award = award_for_period_kind("day").unwrap();
        let mut run = AwardRun::pending(award.slug, "2026-04-14", "day");
        run.status = AwardRunStatus::AwardedDiscordPending;
        run.winner_pubkey = Some(FIRST.into());
        run.loops = Some(1.5);
        run.award_event_id = Some("award-event-id".into());
        repo.upsert_award_run(run).await.unwrap();
        let candidates = FakeCandidates::default();
        let activity = FakeActivity::default();
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 1, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
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
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(outcome.runs[0].winner_pubkey.as_deref(), Some(FIRST));
    });
}

#[test]
fn empty_and_fully_ineligible_fake_results_are_skipped() {
    block_on(async {
        for list in [Vec::new(), vec![candidate(FOUNDER, "founder", 1)]] {
            let repo = FakeRepo::default();
            let candidates = FakeCandidates {
                candidates: list,
                ..Default::default()
            };
            let activity = FakeActivity::default();
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(
                Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
                &repo,
                &candidates,
                &activity,
                &publisher,
                &discord,
            )
            .await
            .unwrap();

            assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
            assert_eq!(*publisher.count.borrow(), 0);
        }
    });
}

#[test]
fn all_inactive_candidates_are_skipped_without_publication() {
    block_on(async {
        let repo = FakeRepo::default();
        let candidates = FakeCandidates {
            candidates: vec![
                candidate(FIRST, "inactive one", 1),
                candidate(SECOND, "inactive two", 2),
            ],
            ..Default::default()
        };
        let activity = FakeActivity {
            latest: HashMap::from([
                (FIRST.into(), video(2026, 1, 1, 0)),
                (SECOND.into(), video(2026, 2, 1, 0)),
            ]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
        assert_eq!(*publisher.count.borrow(), 0);
        assert!(discord.messages.borrow().is_empty());
    });
}

#[test]
fn saves_complete_score_receipt_before_nostr_publication() {
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
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
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
        let operations = repo.operations.borrow();
        let saved = operations
            .iter()
            .position(|value| value == "save-award-run")
            .unwrap();
        let published = operations
            .iter()
            .position(|value| value == "publish-definition")
            .unwrap();
        assert!(saved < published);
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
        let activity = FakeActivity {
            latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
            ..Default::default()
        };
        let publisher = FakePublisher::new(repo.operations.clone());
        let discord = FakeDiscord::default();

        let outcome = execute(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &repo,
            &candidates,
            &activity,
            &publisher,
            &discord,
        )
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
        assert_eq!(run.engagement_tier, Some(1));
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
            let activity = FakeActivity {
                latest: HashMap::from([(FIRST.into(), video(2026, 4, 14, 12))]),
                ..Default::default()
            };
            let publisher = FakePublisher::new(repo.operations.clone());
            let discord = FakeDiscord::default();

            let outcome = execute(
                Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
                &repo,
                &candidates,
                &activity,
                &publisher,
                &discord,
            )
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
            assert!(discord.messages.borrow().is_empty(), "{field}");
        }
    });
}
