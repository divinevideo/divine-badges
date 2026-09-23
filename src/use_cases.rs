use crate::awards::award_for_period_kind;
use crate::clock::{Clock, SystemClock};
use crate::config::AppConfig;
use crate::digest::{digest_campaign, fetch_all_stats};
use crate::discord::{build_announcement_message, build_legacy_announcement_message};
use crate::eligibility::{
    is_candidate_active_for_period, is_diviner_award_excluded_creator,
    DIVINER_AWARD_EXCLUDED_PUBKEYS,
};
use crate::engagement::{broadcast_campaign, winner_campaign};
use crate::error::AppError;
use crate::models::{AwardRun, BadgeDefinitionRecord, DivinerCandidate};
use crate::nostr::{build_badge_award_tags, SignedNostrEvent};
use crate::period::closed_periods_for_tick;
use crate::ports::{
    AwardRepository, BadgePublisher, CampaignClient, CreatorPeriodStatsClient, DiscordClient,
    DivinerCandidatesClient,
};
use crate::state::AwardRunStatus;
use chrono::{DateTime, Utc};

const CANDIDATE_WINDOW: usize = 10;
const DISCORD_DELIVERY_LEASE: chrono::Duration = chrono::Duration::minutes(5);
const DISCORD_WEBHOOK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq)]
pub struct TickOutcome {
    pub runs: Vec<AwardRun>,
}

pub async fn run_award_tick<R, L, P, D, M, S>(
    now: DateTime<Utc>,
    config: &AppConfig,
    repository: &R,
    candidates: &L,
    publisher: &P,
    discord: &D,
    campaigns: &M,
    stats: &S,
) -> Result<TickOutcome, AppError>
where
    R: AwardRepository,
    L: DivinerCandidatesClient,
    P: BadgePublisher,
    D: DiscordClient,
    M: CampaignClient,
    S: CreatorPeriodStatsClient,
{
    run_award_tick_with_clock(
        now,
        &SystemClock,
        config,
        repository,
        candidates,
        publisher,
        discord,
        campaigns,
        stats,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_award_tick_with_clock<R, L, P, D, M, S, C>(
    now: DateTime<Utc>,
    clock: &C,
    config: &AppConfig,
    repository: &R,
    candidates: &L,
    publisher: &P,
    discord: &D,
    campaigns: &M,
    stats: &S,
) -> Result<TickOutcome, AppError>
where
    R: AwardRepository,
    L: DivinerCandidatesClient,
    P: BadgePublisher,
    D: DiscordClient,
    M: CampaignClient,
    S: CreatorPeriodStatsClient,
    C: Clock,
{
    let mut runs = Vec::new();
    let periods = closed_periods_for_tick(now);

    for period in &periods {
        let award = award_for_period_kind(period.kind)
            .ok_or_else(|| AppError::Config(format!("unknown period kind {}", period.kind)))?;
        let seed = BadgeDefinitionRecord::from_award(&award, &config.divine_badge_image_url);
        repository.insert_badge_definition_seed(&seed).await?;

        let mut run = repository
            .upsert_award_run(AwardRun::pending(award.slug, &period.key, period.kind))
            .await?;
        if run.status == AwardRunStatus::Completed {
            runs.push(run);
            continue;
        }
        if matches!(
            run.status,
            AwardRunStatus::Awarded
                | AwardRunStatus::DiscordSending
                | AwardRunStatus::AwardedDiscordPending
        ) {
            let delivered =
                deliver_discord(clock, &award, config, repository, discord, run).await?;
            if delivered.status == AwardRunStatus::Completed {
                notify_engagement(clock, config, repository, campaigns, &delivered).await;
            }
            runs.push(delivered);
            continue;
        }

        if run.winner_pubkey.is_none() {
            let ranked = match candidates
                .ranked_candidates(
                    period.start,
                    period.end,
                    CANDIDATE_WINDOW + DIVINER_AWARD_EXCLUDED_PUBKEYS.len(),
                )
                .await
            {
                Ok(creators) => creators,
                Err(AppError::EmptyCandidates(_)) => {
                    runs.push(
                        repository
                            .mark_skipped_inactive(award.slug, &period.key)
                            .await?,
                    );
                    continue;
                }
                Err(err) => {
                    runs.push(
                        repository
                            .mark_fetch_failed(award.slug, &period.key, &err.to_string())
                            .await?,
                    );
                    continue;
                }
            };
            let winner = ranked.into_iter().find(|creator| {
                !is_diviner_award_excluded_creator(&creator.pubkey)
                    && is_candidate_active_for_period(
                        creator.latest_eligible_publication_at,
                        period.end,
                    )
            });
            let Some(winner) = winner else {
                runs.push(
                    repository
                        .mark_skipped_inactive(award.slug, &period.key)
                        .await?,
                );
                continue;
            };
            let proposed = match enrich_run_with_winner(run, &winner) {
                Ok(proposed) => proposed,
                Err(err) => {
                    runs.push(
                        repository
                            .mark_fetch_failed(award.slug, &period.key, &err.to_string())
                            .await?,
                    );
                    continue;
                }
            };
            run = repository.claim_winner(&proposed).await?;
        }

        let badge_definition = repository
            .load_badge_definition(award.slug)
            .await?
            .unwrap_or_else(|| seed.clone());
        let badge_definition = if badge_definition.definition_coordinate.is_none() {
            match publisher
                .publish_definition(
                    &award,
                    &badge_definition.image_url,
                    &badge_definition.thumb_url,
                )
                .await
            {
                Ok(result) => {
                    let mut updated = badge_definition.clone();
                    updated.definition_event_id = Some(result.definition_event_id);
                    updated.definition_coordinate = Some(result.definition_coordinate);
                    repository.save_badge_definition(&updated).await?;
                    updated
                }
                Err(err) => {
                    runs.push(
                        repository
                            .mark_definition_failed(award.slug, &period.key, &err.to_string())
                            .await?,
                    );
                    continue;
                }
            }
        } else {
            badge_definition
        };
        let badge_coordinate = badge_definition
            .definition_coordinate
            .ok_or_else(|| AppError::Relay("missing badge coordinate".into()))?;
        let winner_pubkey = run
            .winner_pubkey
            .clone()
            .ok_or_else(|| AppError::Repository("claimed run is missing winner pubkey".into()))?;

        let prepared = if run.prepared_award_event.is_some() {
            stored_prepared_event(&run, &badge_coordinate, &winner_pubkey, &period.key)
        } else {
            let proposed =
                match publisher.prepare_award(&badge_coordinate, &winner_pubkey, &period.key) {
                    Ok(event) => event,
                    Err(err) => {
                        runs.push(
                            repository
                                .mark_preparation_failed(award.slug, &period.key, &err.to_string())
                                .await?,
                        );
                        continue;
                    }
                };
            run = repository
                .claim_prepared_award(award.slug, &period.key, &proposed)
                .await?;
            stored_prepared_event(&run, &badge_coordinate, &winner_pubkey, &period.key)
        };
        let prepared = match prepared {
            Ok(event) => event,
            Err(err) => {
                runs.push(
                    repository
                        .mark_award_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
        };

        let published_event_id = match publisher.publish_prepared_award(&prepared).await {
            Ok(event_id) if event_id == prepared.id => event_id,
            Ok(event_id) => {
                let err = AppError::Relay(format!(
                    "relay returned event id {event_id} for prepared event {}",
                    prepared.id
                ));
                runs.push(
                    repository
                        .mark_award_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
            Err(err) => {
                runs.push(
                    repository
                        .mark_award_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
        };
        run = repository
            .mark_awarded(award.slug, &period.key, &published_event_id)
            .await?;
        let delivered = deliver_discord(clock, &award, config, repository, discord, run).await?;
        if delivered.status == AwardRunStatus::Completed {
            notify_engagement(clock, config, repository, campaigns, &delivered).await;
        }
        runs.push(delivered);
    }

    if let Some(day) = periods.iter().find(|period| period.kind == "day") {
        run_creator_digest(clock, config, repository, stats, campaigns, &day.key).await;
    }

    Ok(TickOutcome { runs })
}

fn enrich_run_with_winner(
    mut run: AwardRun,
    winner: &DivinerCandidate,
) -> Result<AwardRun, AppError> {
    run.winner_pubkey = Some(winner.pubkey.clone());
    run.winner_display_name = Some(winner.best_display_name());
    run.winner_name = Some(winner.name.clone());
    run.winner_nip05 = winner.nip05.clone();
    run.winner_picture = Some(winner.picture.clone());
    run.latest_eligible_publication_at = Some(winner.latest_eligible_publication_at);
    run.loops = Some(winner.loops);
    run.views = Some(js_safe_i64("views", winner.views)?);
    run.unique_viewers = Some(js_safe_i64("unique_viewers", winner.unique_viewers)?);
    run.videos_with_views = Some(js_safe_i64("videos_with_views", winner.videos_with_views)?);
    run.positive_reactors = Some(js_safe_i64("positive_reactors", winner.positive_reactors)?);
    run.distinct_commenters = Some(js_safe_i64(
        "distinct_commenters",
        winner.distinct_commenters,
    )?);
    run.distinct_reposters = Some(js_safe_i64(
        "distinct_reposters",
        winner.distinct_reposters,
    )?);
    run.distinct_positive_engagers = Some(js_safe_i64(
        "distinct_positive_engagers",
        winner.distinct_positive_engagers,
    )?);
    run.engagement_tier = Some(i64::from(winner.engagement_tier));
    run.engagement_rate = Some(winner.engagement_rate);
    run.score = Some(winner.score);
    Ok(run)
}

const JS_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

fn js_safe_i64(field: &str, value: u64) -> Result<i64, AppError> {
    if value > JS_SAFE_INTEGER_MAX {
        return Err(AppError::Api(format!(
            "Diviner candidate {field} exceeds the D1 exact integer maximum {JS_SAFE_INTEGER_MAX}"
        )));
    }
    i64::try_from(value)
        .map_err(|_| AppError::Api(format!("Diviner candidate {field} is out of range")))
}

fn stored_prepared_event(
    run: &AwardRun,
    badge_coordinate: &str,
    winner_pubkey: &str,
    period_key: &str,
) -> Result<SignedNostrEvent, AppError> {
    let serialized = run.prepared_award_event.as_deref().ok_or_else(|| {
        AppError::Repository("claimed run is missing prepared award event".into())
    })?;
    let event: SignedNostrEvent = serde_json::from_str(serialized)
        .map_err(|err| AppError::Repository(format!("invalid prepared award event: {err}")))?;
    if event.kind != 8 {
        return Err(AppError::Repository(format!(
            "prepared award event has kind {}, expected 8",
            event.kind
        )));
    }
    if !event.content.is_empty()
        || event.tags != build_badge_award_tags(badge_coordinate, winner_pubkey, period_key)
    {
        return Err(AppError::Repository(
            "prepared award event payload does not match the canonical award receipt".into(),
        ));
    }
    if run.award_event_id.as_deref() != Some(event.id.as_str()) {
        return Err(AppError::Repository(
            "prepared award event ID does not match stored award event ID".into(),
        ));
    }
    Ok(event)
}

fn announcement_message(
    award: &crate::awards::AwardDefinition,
    config: &AppConfig,
    run: &AwardRun,
) -> Result<String, AppError> {
    let winner_pubkey = run
        .winner_pubkey
        .as_deref()
        .ok_or_else(|| AppError::Discord("missing winner pubkey".into()))?;
    let winner_display = run
        .winner_display_name
        .as_deref()
        .or(run.winner_name.as_deref())
        .unwrap_or(winner_pubkey);
    let creator_link = config.creator_link(run.winner_nip05.as_deref(), winner_pubkey);
    match (
        run.positive_reactors,
        run.distinct_commenters,
        run.distinct_reposters,
        run.unique_viewers,
    ) {
        (Some(positive_reactors), Some(commenters), Some(reposts), Some(unique_viewers)) => {
            build_announcement_message(
                award.badge_name,
                winner_display,
                receipt_count(positive_reactors, "positive reactors")?,
                receipt_count(commenters, "commenters")?,
                receipt_count(reposts, "reposts")?,
                receipt_count(unique_viewers, "unique viewers")?,
                &creator_link,
            )
        }
        _ => build_legacy_announcement_message(award.badge_name, winner_display, &creator_link),
    }
}

fn receipt_count(value: i64, field: &str) -> Result<u64, AppError> {
    u64::try_from(value)
        .map_err(|_| AppError::Discord(format!("invalid negative {field} in stored award receipt")))
}

async fn deliver_discord<R, D, C>(
    clock: &C,
    award: &crate::awards::AwardDefinition,
    config: &AppConfig,
    repository: &R,
    discord: &D,
    run: AwardRun,
) -> Result<AwardRun, AppError>
where
    R: AwardRepository,
    D: DiscordClient,
    C: Clock,
{
    if run.status == AwardRunStatus::Completed {
        return Ok(run);
    }

    let claim_token = new_discord_claim_token()?;
    let claim_time = clock.now();
    let lease_expires_at = claim_time
        .checked_add_signed(DISCORD_DELIVERY_LEASE)
        .ok_or_else(|| AppError::Repository("Discord delivery lease overflow".into()))?;
    let claim = repository
        .claim_discord_delivery(
            &run.award_slug,
            &run.period_key,
            &claim_token,
            claim_time,
            lease_expires_at,
        )
        .await?;
    if !claim.acquired {
        return Ok(claim.run);
    }

    let message = match announcement_message(award, config, &claim.run) {
        Ok(message) => message,
        Err(err) => {
            return repository
                .mark_discord_pending(
                    &claim.run.award_slug,
                    &claim.run.period_key,
                    &claim_token,
                    &err.to_string(),
                )
                .await;
        }
    };
    // The lease prevents overlapping live deliveries. Discord HTTP acceptance and the D1
    // completion write cannot be atomic, so a crash between them can still duplicate after expiry.
    match discord
        .post_message(&message, DISCORD_WEBHOOK_TIMEOUT)
        .await
    {
        Ok(()) => {
            repository
                .mark_completed(&run.award_slug, &run.period_key, &claim_token)
                .await
        }
        Err(err) => {
            repository
                .mark_discord_pending(
                    &run.award_slug,
                    &run.period_key,
                    &claim_token,
                    &err.to_string(),
                )
                .await
        }
    }
}

/// Create the award's campaigns once, after the run completes.
///
/// Failures are logged and swallowed. A missed notification is invisible and
/// recoverable tomorrow; failing the tick would risk re-running the award.
/// The claim is taken first: a campaign that was created but whose response
/// was lost must not be created twice, and divine-engagement is idempotent on
/// `automationKey` as the second line of defence.
async fn notify_engagement<R, N, C>(
    clock: &C,
    config: &AppConfig,
    repository: &R,
    campaigns: &N,
    run: &AwardRun,
) where
    R: AwardRepository,
    N: CampaignClient,
    C: Clock,
{
    if config.engagement_api_base_url.is_none() {
        return;
    }

    match repository
        .claim_push_notification(&run.award_slug, &run.period_key, clock.now())
        .await
    {
        Ok(true) => {}
        Ok(false) => return,
        Err(_) => return,
    }

    for campaign in [winner_campaign(run), broadcast_campaign(run)]
        .into_iter()
        .flatten()
    {
        if let Err(err) = campaigns.create_campaign(&campaign).await {
            log_engagement_error(&err);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn log_engagement_error(err: &AppError) {
    worker::console_error!("engagement campaign creation failed: {}", err);
}

#[cfg(not(target_arch = "wasm32"))]
fn log_engagement_error(_err: &AppError) {}

/// Send the day's creator digest, once.
///
/// The stats fetch happens before the claim: claiming first would burn the day
/// on one transient 500 and never retry. A failed fetch leaves the day
/// unclaimed. A failed campaign creation is logged and swallowed, so a digest
/// failure never fails the award tick.
///
/// The tick is hourly and every tick of a day sees the same closed day, so the
/// already-sent read comes first. Without it the other 23 ticks would each
/// walk the whole stats endpoint before the claim told them there was nothing
/// to do.
async fn run_creator_digest<R, S, M, C>(
    clock: &C,
    config: &AppConfig,
    repository: &R,
    stats: &S,
    campaigns: &M,
    period_key: &str,
) where
    R: AwardRepository,
    S: CreatorPeriodStatsClient,
    M: CampaignClient,
    C: Clock,
{
    if !config.digest_enabled || config.engagement_api_base_url.is_none() {
        return;
    }

    // A read failure is not a reason to skip the day: the claim below still
    // makes the send once-only, so falling through costs a walk, not a
    // duplicate notification.
    match repository.digest_already_notified(period_key).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => log_digest_error(&err),
    }

    let all_stats = match fetch_all_stats(stats, period_key).await {
        Ok(all_stats) => all_stats,
        Err(err) => {
            log_digest_error(&err);
            return;
        }
    };

    match repository
        .claim_digest_notification(period_key, clock.now())
        .await
    {
        Ok(true) => {}
        Ok(false) => return,
        Err(err) => {
            log_digest_error(&err);
            return;
        }
    }

    let Some(campaign) = digest_campaign(period_key, all_stats) else {
        return;
    };
    if let Err(err) = campaigns.create_campaign(&campaign).await {
        log_digest_error(&err);
    }
}

#[cfg(target_arch = "wasm32")]
fn log_digest_error(err: &AppError) {
    worker::console_error!("creator digest failed: {}", err);
}

#[cfg(not(target_arch = "wasm32"))]
fn log_digest_error(_err: &AppError) {}

fn new_discord_claim_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|err| {
        AppError::Repository(format!("Discord claim token generation failed: {err}"))
    })?;
    Ok(hex::encode(bytes))
}
