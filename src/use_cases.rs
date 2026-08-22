use crate::awards::award_for_period_kind;
use crate::clock::{Clock, SystemClock};
use crate::config::AppConfig;
use crate::discord::build_announcement_message;
use crate::eligibility::{is_candidate_active_for_period, is_diviner_award_excluded_creator};
use crate::error::AppError;
use crate::models::{AwardRun, BadgeDefinitionRecord, DivinerCandidate};
use crate::nostr::{build_badge_award_tags, SignedNostrEvent};
use crate::period::closed_periods_for_tick;
use crate::ports::{AwardRepository, BadgePublisher, DiscordClient, DivinerCandidatesClient};
use crate::state::AwardRunStatus;
use chrono::{DateTime, Utc};

const CANDIDATE_WINDOW: usize = 10;
const DISCORD_DELIVERY_LEASE: chrono::Duration = chrono::Duration::minutes(5);
const DISCORD_WEBHOOK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4 * 60);

#[derive(Debug, Clone, PartialEq)]
pub struct TickOutcome {
    pub runs: Vec<AwardRun>,
}

pub async fn run_award_tick<R, L, P, D>(
    now: DateTime<Utc>,
    config: &AppConfig,
    repository: &R,
    candidates: &L,
    publisher: &P,
    discord: &D,
) -> Result<TickOutcome, AppError>
where
    R: AwardRepository,
    L: DivinerCandidatesClient,
    P: BadgePublisher,
    D: DiscordClient,
{
    run_award_tick_with_clock(
        now,
        &SystemClock,
        config,
        repository,
        candidates,
        publisher,
        discord,
    )
    .await
}

pub async fn run_award_tick_with_clock<R, L, P, D, C>(
    now: DateTime<Utc>,
    clock: &C,
    config: &AppConfig,
    repository: &R,
    candidates: &L,
    publisher: &P,
    discord: &D,
) -> Result<TickOutcome, AppError>
where
    R: AwardRepository,
    L: DivinerCandidatesClient,
    P: BadgePublisher,
    D: DiscordClient,
    C: Clock,
{
    let mut runs = Vec::new();

    for period in closed_periods_for_tick(now) {
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
            runs.push(deliver_discord(clock, &award, config, repository, discord, run).await?);
            continue;
        }

        if run.winner_pubkey.is_none() {
            let ranked = match candidates
                .ranked_candidates(period.start, period.end, CANDIDATE_WINDOW)
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
        runs.push(deliver_discord(clock, &award, config, repository, discord, run).await?);
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
    Ok(build_announcement_message(
        award.badge_name,
        winner_display,
        run.loops.unwrap_or_default(),
        &config.creator_link(run.winner_nip05.as_deref(), winner_pubkey),
    ))
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

    let message = announcement_message(award, config, &claim.run)?;
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

fn new_discord_claim_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|err| {
        AppError::Repository(format!("Discord claim token generation failed: {err}"))
    })?;
    Ok(hex::encode(bytes))
}
