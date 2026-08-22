use crate::awards::award_for_period_kind;
use crate::config::AppConfig;
use crate::discord::build_announcement_message;
use crate::eligibility::{is_active_creator, is_diviner_award_excluded_creator};
use crate::error::AppError;
use crate::models::{AwardRun, BadgeDefinitionRecord, DivinerCandidate};
use crate::period::closed_periods_for_tick;
use crate::ports::{
    AwardRepository, BadgePublisher, CreatorActivityClient, DiscordClient, DivinerCandidatesClient,
};
use crate::state::AwardRunStatus;
use chrono::{DateTime, Utc};

const CANDIDATE_WINDOW: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct TickOutcome {
    pub runs: Vec<AwardRun>,
}

pub async fn run_award_tick<R, L, A, P, D>(
    now: DateTime<Utc>,
    config: &AppConfig,
    repository: &R,
    candidates: &L,
    activity: &A,
    publisher: &P,
    discord: &D,
) -> Result<TickOutcome, AppError>
where
    R: AwardRepository,
    L: DivinerCandidatesClient,
    A: CreatorActivityClient,
    P: BadgePublisher,
    D: DiscordClient,
{
    let mut runs = Vec::new();

    for period in closed_periods_for_tick(now) {
        let award = award_for_period_kind(period.kind)
            .ok_or_else(|| AppError::Config(format!("unknown period kind {}", period.kind)))?;

        let seed = BadgeDefinitionRecord::from_award(&award, &config.divine_badge_image_url);
        repository.insert_badge_definition_seed(&seed).await?;

        let run = repository
            .upsert_award_run(AwardRun::pending(award.slug, &period.key, period.kind))
            .await?;

        if run.status == AwardRunStatus::Completed {
            runs.push(run);
            continue;
        }

        if matches!(
            run.status,
            AwardRunStatus::Awarded | AwardRunStatus::AwardedDiscordPending
        ) {
            let completed = retry_discord_only(&award, config, repository, discord, run).await?;
            runs.push(completed);
            continue;
        }

        let ranked = match candidates
            .ranked_candidates(period.start, period.end, CANDIDATE_WINDOW)
            .await
        {
            Ok(creators) => creators,
            Err(err) => {
                runs.push(
                    repository
                        .mark_fetch_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
        };

        let mut winner = None;
        for creator in ranked {
            if is_diviner_award_excluded_creator(&creator.pubkey) {
                continue;
            }

            let latest_video = match activity
                .latest_video_before(&creator.pubkey, period.end)
                .await
            {
                Ok(video) => video,
                Err(err) => {
                    runs.push(
                        repository
                            .mark_fetch_failed(award.slug, &period.key, &err.to_string())
                            .await?,
                    );
                    winner = None;
                    break;
                }
            };

            if latest_video
                .as_ref()
                .map(|video| is_active_creator(period.end, video))
                .unwrap_or(false)
            {
                winner = Some(creator);
                break;
            }
        }

        let Some(winner) = winner else {
            if runs
                .iter()
                .any(|run| run.award_slug == award.slug && run.period_key == period.key)
            {
                continue;
            }

            runs.push(
                repository
                    .mark_skipped_inactive(award.slug, &period.key)
                    .await?,
            );
            continue;
        };

        let enriched_run = match enrich_run_with_winner(run, &winner) {
            Ok(run) => run,
            Err(err) => {
                runs.push(
                    repository
                        .mark_fetch_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
        };
        repository.save_award_run(&enriched_run).await?;

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
            .clone()
            .ok_or_else(|| AppError::Relay("missing badge coordinate".into()))?;

        let award_event_id = match publisher
            .publish_award(&badge_coordinate, &winner.pubkey, &period.key)
            .await
        {
            Ok(event_id) => event_id,
            Err(err) => {
                runs.push(
                    repository
                        .mark_award_failed(award.slug, &period.key, &err.to_string())
                        .await?,
                );
                continue;
            }
        };

        repository
            .mark_awarded(award.slug, &period.key, &award_event_id)
            .await?;

        let creator_link = config.creator_link(winner.nip05.as_deref(), &winner.pubkey);
        let message = build_announcement_message(
            award.badge_name,
            &winner.best_display_name(),
            winner.loops,
            &creator_link,
        );

        match discord.post_message(&message).await {
            Ok(()) => runs.push(repository.mark_completed(award.slug, &period.key).await?),
            Err(err) => runs.push(
                repository
                    .mark_discord_pending(award.slug, &period.key, &err.to_string())
                    .await?,
            ),
        }
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

async fn retry_discord_only<R, D>(
    award: &crate::awards::AwardDefinition,
    config: &AppConfig,
    repository: &R,
    discord: &D,
    run: AwardRun,
) -> Result<AwardRun, AppError>
where
    R: AwardRepository,
    D: DiscordClient,
{
    let winner_pubkey = run
        .winner_pubkey
        .clone()
        .ok_or_else(|| AppError::Discord("missing winner pubkey".into()))?;
    let winner_display = run
        .winner_display_name
        .clone()
        .or_else(|| run.winner_name.clone())
        .unwrap_or_else(|| winner_pubkey.clone());
    let message = build_announcement_message(
        award.badge_name,
        &winner_display,
        run.loops.unwrap_or_default(),
        &config.creator_link(run.winner_nip05.as_deref(), &winner_pubkey),
    );

    discord.post_message(&message).await?;
    repository
        .mark_completed(&run.award_slug, &run.period_key)
        .await
}
