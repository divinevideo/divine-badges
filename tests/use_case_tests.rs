use std::cell::RefCell;
use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use divine_badges::awards::award_for_period_kind;
use divine_badges::config::AppConfig;
use divine_badges::error::AppError;
use divine_badges::models::{
    AwardRun, BadgeDefinitionRecord, CreatorLatestVideo, LeaderboardCreator,
};
use divine_badges::ports::{
    AwardRepository, BadgePublisher, CreatorActivityClient, DiscordClient, LeaderboardClient,
};
use divine_badges::state::AwardRunStatus;
use divine_badges::use_cases::run_award_tick;
use futures::executor::block_on;

#[derive(Default)]
struct FakeRepo {
    badge_definitions: RefCell<HashMap<String, BadgeDefinitionRecord>>,
    runs: RefCell<HashMap<(String, String), AwardRun>>,
}

#[async_trait(?Send)]
impl AwardRepository for FakeRepo {
    async fn insert_badge_definition_seed(
        &self,
        record: &BadgeDefinitionRecord,
    ) -> Result<(), AppError> {
        self.badge_definitions
            .borrow_mut()
            .entry(record.award_slug.clone())
            .or_insert_with(|| record.clone());
        Ok(())
    }

    async fn load_badge_definition(
        &self,
        award_slug: &str,
    ) -> Result<Option<BadgeDefinitionRecord>, AppError> {
        Ok(self.badge_definitions.borrow().get(award_slug).cloned())
    }

    async fn save_badge_definition(&self, record: &BadgeDefinitionRecord) -> Result<(), AppError> {
        self.badge_definitions
            .borrow_mut()
            .insert(record.award_slug.clone(), record.clone());
        Ok(())
    }

    async fn upsert_award_run(&self, run: AwardRun) -> Result<AwardRun, AppError> {
        let key = (run.award_slug.clone(), run.period_key.clone());
        let mut runs = self.runs.borrow_mut();
        let current = runs.entry(key).or_insert(run);
        Ok(current.clone())
    }

    async fn save_award_run(&self, run: &AwardRun) -> Result<AwardRun, AppError> {
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
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError> {
        self.update_status(
            award_slug,
            period_key,
            AwardRunStatus::FailedFetch,
            Some(error_message),
        )
    }

    async fn mark_definition_failed(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError> {
        self.update_status(
            award_slug,
            period_key,
            AwardRunStatus::FailedDefinition,
            Some(error_message),
        )
    }

    async fn mark_awarded(
        &self,
        award_slug: &str,
        period_key: &str,
        award_event_id: &str,
    ) -> Result<AwardRun, AppError> {
        let mut run = self.update_status(award_slug, period_key, AwardRunStatus::Awarded, None)?;
        run.award_event_id = Some(award_event_id.to_string());
        self.runs.borrow_mut().insert(
            (award_slug.to_string(), period_key.to_string()),
            run.clone(),
        );
        Ok(run)
    }

    async fn mark_award_failed(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError> {
        self.update_status(
            award_slug,
            period_key,
            AwardRunStatus::FailedAward,
            Some(error_message),
        )
    }

    async fn mark_discord_pending(
        &self,
        award_slug: &str,
        period_key: &str,
        error_message: &str,
    ) -> Result<AwardRun, AppError> {
        self.update_status(
            award_slug,
            period_key,
            AwardRunStatus::AwardedDiscordPending,
            Some(error_message),
        )
    }

    async fn mark_completed(
        &self,
        award_slug: &str,
        period_key: &str,
    ) -> Result<AwardRun, AppError> {
        let mut run =
            self.update_status(award_slug, period_key, AwardRunStatus::Completed, None)?;
        run.discord_message_sent = true;
        self.runs.borrow_mut().insert(
            (award_slug.to_string(), period_key.to_string()),
            run.clone(),
        );
        Ok(run)
    }

    async fn mark_skipped_inactive(
        &self,
        award_slug: &str,
        period_key: &str,
    ) -> Result<AwardRun, AppError> {
        self.update_status(
            award_slug,
            period_key,
            AwardRunStatus::SkippedInactive,
            None,
        )
    }
}

impl FakeRepo {
    fn update_status(
        &self,
        award_slug: &str,
        period_key: &str,
        status: AwardRunStatus,
        error_message: Option<&str>,
    ) -> Result<AwardRun, AppError> {
        let key = (award_slug.to_string(), period_key.to_string());
        let mut runs = self.runs.borrow_mut();
        let run = runs
            .get_mut(&key)
            .ok_or_else(|| AppError::Repository("missing run".into()))?;
        run.status = status;
        run.error_message = error_message.map(ToString::to_string);
        Ok(run.clone())
    }
}

struct FakeLeaderboard {
    creators: Vec<LeaderboardCreator>,
}

#[async_trait(?Send)]
impl LeaderboardClient for FakeLeaderboard {
    async fn ranked_creators(
        &self,
        _period: &str,
        _candidate_window: usize,
    ) -> Result<Vec<LeaderboardCreator>, AppError> {
        Ok(self.creators.clone())
    }
}

struct FakeActivity {
    latest_by_pubkey: HashMap<String, Option<CreatorLatestVideo>>,
}

#[async_trait(?Send)]
impl CreatorActivityClient for FakeActivity {
    async fn latest_video(&self, pubkey: &str) -> Result<Option<CreatorLatestVideo>, AppError> {
        Ok(self.latest_by_pubkey.get(pubkey).cloned().flatten())
    }
}

#[derive(Default)]
struct FakePublisher {
    publish_count: RefCell<usize>,
}

#[async_trait(?Send)]
impl BadgePublisher for FakePublisher {
    async fn publish_definition(
        &self,
        award: &divine_badges::awards::AwardDefinition,
        _image_url: &str,
        _thumb_url: &str,
    ) -> Result<divine_badges::nostr::DefinitionPublishResult, AppError> {
        *self.publish_count.borrow_mut() += 1;
        Ok(divine_badges::nostr::DefinitionPublishResult {
            definition_event_id: format!("definition-{}", award.slug),
            definition_coordinate: format!("30009:issuerpubkey:{}", award.d_tag),
        })
    }

    async fn publish_award(
        &self,
        _badge_coordinate: &str,
        _winner_pubkey: &str,
        _period_key: &str,
    ) -> Result<String, AppError> {
        *self.publish_count.borrow_mut() += 1;
        Ok("award-event-id".into())
    }
}

#[derive(Default)]
struct FakeDiscord {
    send_count: RefCell<usize>,
}

#[async_trait(?Send)]
impl DiscordClient for FakeDiscord {
    async fn post_message(&self, _message: &str) -> Result<(), AppError> {
        *self.send_count.borrow_mut() += 1;
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

fn fake_creator(pubkey: &str, display_name: &str, loops: f64) -> LeaderboardCreator {
    LeaderboardCreator {
        pubkey: pubkey.into(),
        display_name: display_name.into(),
        name: display_name.into(),
        nip05: None,
        picture: String::new(),
        loops,
        views: loops.round() as i64,
        unique_viewers: 1,
        videos_with_views: 1,
    }
}

#[test]
fn completed_run_skips_duplicate_award_publish() {
    block_on(async {
        let repo = FakeRepo::default();
        let award = award_for_period_kind("day").unwrap();
        repo.insert_badge_definition_seed(&BadgeDefinitionRecord::from_award(
            &award,
            "https://cdn.divine.video/logo.png",
        ))
        .await
        .unwrap();
        repo.upsert_award_run(AwardRun::completed(
            award.slug,
            "2026-04-14",
            "day",
            "award-event-id",
        ))
        .await
        .unwrap();

        let leaderboard = FakeLeaderboard { creators: vec![] };
        let activity = FakeActivity {
            latest_by_pubkey: HashMap::new(),
        };
        let publisher = FakePublisher::default();
        let discord = FakeDiscord::default();

        let outcome = run_award_tick(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &config(),
            &repo,
            &leaderboard,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs.len(), 1);
        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(*publisher.publish_count.borrow(), 0);
        assert_eq!(*discord.send_count.borrow(), 0);
    });
}

#[test]
fn inactive_candidates_mark_run_skipped_without_publishing() {
    block_on(async {
        let repo = FakeRepo::default();
        let leaderboard = FakeLeaderboard {
            creators: vec![
                fake_creator("archivepubkey", "KingBach", 1100.0),
                fake_creator("archivepubkey2", "ThomasSanders", 1000.0),
            ],
        };
        let activity = FakeActivity {
            latest_by_pubkey: HashMap::from([
                (
                    "archivepubkey".into(),
                    Some(CreatorLatestVideo {
                        published_at: Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap(),
                    }),
                ),
                (
                    "archivepubkey2".into(),
                    Some(CreatorLatestVideo {
                        published_at: Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap(),
                    }),
                ),
            ]),
        };
        let publisher = FakePublisher::default();
        let discord = FakeDiscord::default();

        let outcome = run_award_tick(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &config(),
            &repo,
            &leaderboard,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs.len(), 1);
        assert_eq!(outcome.runs[0].status, AwardRunStatus::SkippedInactive);
        assert_eq!(*publisher.publish_count.borrow(), 0);
        assert_eq!(*discord.send_count.borrow(), 0);
    });
}

#[test]
fn winning_run_persists_winner_nip05() {
    block_on(async {
        let repo = FakeRepo::default();
        let leaderboard = FakeLeaderboard {
            creators: vec![LeaderboardCreator {
                pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                display_name: "rabble".into(),
                name: "rabble".into(),
                nip05: Some("rabble@divine.video".into()),
                picture: "https://cdn.divine.video/rabble.png".into(),
                loops: 321.0,
                views: 321,
                unique_viewers: 12,
                videos_with_views: 3,
            }],
        };
        let activity = FakeActivity {
            latest_by_pubkey: HashMap::from([(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                Some(CreatorLatestVideo {
                    published_at: Utc.with_ymd_and_hms(2026, 4, 14, 12, 0, 0).unwrap(),
                }),
            )]),
        };
        let publisher = FakePublisher::default();
        let discord = FakeDiscord::default();

        let outcome = run_award_tick(
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 5, 0).unwrap(),
            &config(),
            &repo,
            &leaderboard,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs.len(), 1);
        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(
            outcome.runs[0].winner_nip05.as_deref(),
            Some("rabble@divine.video")
        );
    });
}

#[test]
fn discord_pending_run_retries_only_discord() {
    block_on(async {
        let repo = FakeRepo::default();
        let award = award_for_period_kind("day").unwrap();
        repo.insert_badge_definition_seed(&BadgeDefinitionRecord::published(
            &award,
            "https://cdn.divine.video/logo.png",
            "definition-id",
            "30009:issuerpubkey:diviner-of-the-day",
        ))
        .await
        .unwrap();
        let mut run = AwardRun::pending(award.slug, "2026-04-14", "day");
        run.status = AwardRunStatus::AwardedDiscordPending;
        run.winner_pubkey = Some("winnerpubkey".into());
        run.winner_display_name = Some("winner".into());
        run.winner_nip05 = Some("winner@divine.video".into());
        run.loops = Some(136.0);
        run.award_event_id = Some("award-event-id".into());
        repo.upsert_award_run(run).await.unwrap();

        let leaderboard = FakeLeaderboard { creators: vec![] };
        let activity = FakeActivity {
            latest_by_pubkey: HashMap::new(),
        };
        let publisher = FakePublisher::default();
        let discord = FakeDiscord::default();

        let outcome = run_award_tick(
            Utc.with_ymd_and_hms(2026, 4, 15, 1, 5, 0).unwrap(),
            &config(),
            &repo,
            &leaderboard,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .unwrap();

        assert_eq!(outcome.runs.len(), 1);
        assert_eq!(outcome.runs[0].status, AwardRunStatus::Completed);
        assert_eq!(*publisher.publish_count.borrow(), 0);
        assert_eq!(*discord.send_count.borrow(), 1);
    });
}
