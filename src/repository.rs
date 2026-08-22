use crate::models::AwardRun;

#[derive(Debug, Clone, PartialEq)]
pub enum AwardRunSqlValue {
    Null,
    Text(String),
    Integer(i64),
    Real(f64),
}

pub fn is_d1_safe_integer(value: i64) -> bool {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
    (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value)
}

pub fn award_run_unique_index_sql() -> &'static str {
    "CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx ON award_runs (award_slug, period_key);"
}

pub fn save_badge_definition_sql() -> &'static str {
    "UPDATE badge_definitions SET definition_event_id = ?1, definition_coordinate = ?2, published_at = ?3, updated_at = ?4 WHERE award_slug = ?5;"
}

pub fn recent_completed_runs_sql() -> &'static str {
    "SELECT award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, latest_eligible_publication_at, loops, views, unique_viewers, videos_with_views, positive_reactors, distinct_commenters, distinct_reposters, distinct_positive_engagers, engagement_tier, engagement_rate, score, award_event_id, prepared_award_event, discord_message_sent, status, error_message FROM award_runs WHERE award_slug = ?1 AND status = 'completed' ORDER BY period_key DESC LIMIT ?2"
}

pub fn award_run_by_key_sql() -> &'static str {
    "SELECT award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, latest_eligible_publication_at, loops, views, unique_viewers, videos_with_views, positive_reactors, distinct_commenters, distinct_reposters, distinct_positive_engagers, engagement_tier, engagement_rate, score, award_event_id, prepared_award_event, discord_message_sent, status, error_message FROM award_runs WHERE award_slug = ?1 AND period_key = ?2"
}

pub fn upsert_award_run_sql() -> &'static str {
    "INSERT INTO award_runs (award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, latest_eligible_publication_at, loops, views, unique_viewers, videos_with_views, positive_reactors, distinct_commenters, distinct_reposters, distinct_positive_engagers, engagement_tier, engagement_rate, score, award_event_id, prepared_award_event, discord_message_sent, status, error_message, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27) ON CONFLICT(award_slug, period_key) DO NOTHING"
}

pub fn save_award_run_sql() -> &'static str {
    "UPDATE award_runs SET period_type = ?1, winner_pubkey = ?2, winner_display_name = ?3, winner_name = ?4, winner_nip05 = ?5, winner_picture = ?6, latest_eligible_publication_at = ?7, loops = ?8, views = ?9, unique_viewers = ?10, videos_with_views = ?11, positive_reactors = ?12, distinct_commenters = ?13, distinct_reposters = ?14, distinct_positive_engagers = ?15, engagement_tier = ?16, engagement_rate = ?17, score = ?18, award_event_id = ?19, prepared_award_event = ?20, discord_message_sent = ?21, status = ?22, error_message = ?23, updated_at = ?24 WHERE award_slug = ?25 AND period_key = ?26"
}

pub fn claim_winner_sql() -> &'static str {
    "UPDATE award_runs SET winner_pubkey = ?1, winner_display_name = ?2, winner_name = ?3, winner_nip05 = ?4, winner_picture = ?5, latest_eligible_publication_at = ?6, loops = ?7, views = ?8, unique_viewers = ?9, videos_with_views = ?10, positive_reactors = ?11, distinct_commenters = ?12, distinct_reposters = ?13, distinct_positive_engagers = ?14, engagement_tier = ?15, engagement_rate = ?16, score = ?17, error_message = NULL, updated_at = ?18 WHERE award_slug = ?19 AND period_key = ?20 AND winner_pubkey IS NULL"
}

pub fn claim_prepared_award_sql() -> &'static str {
    "UPDATE award_runs SET prepared_award_event = ?1, award_event_id = ?2, status = 'award_prepared', error_message = NULL, updated_at = ?3 WHERE award_slug = ?4 AND period_key = ?5 AND prepared_award_event IS NULL"
}

pub fn award_run_insert_bindings(run: &AwardRun, now: &str) -> Vec<AwardRunSqlValue> {
    vec![
        text(&run.award_slug),
        text(&run.period_key),
        text(&run.period_type),
        optional_text(&run.winner_pubkey),
        optional_text(&run.winner_display_name),
        optional_text(&run.winner_name),
        optional_text(&run.winner_nip05),
        optional_text(&run.winner_picture),
        optional_datetime(run.latest_eligible_publication_at),
        optional_real(run.loops),
        optional_integer(run.views),
        optional_integer(run.unique_viewers),
        optional_integer(run.videos_with_views),
        optional_integer(run.positive_reactors),
        optional_integer(run.distinct_commenters),
        optional_integer(run.distinct_reposters),
        optional_integer(run.distinct_positive_engagers),
        optional_integer(run.engagement_tier),
        optional_real(run.engagement_rate),
        optional_real(run.score),
        optional_text(&run.award_event_id),
        optional_text(&run.prepared_award_event),
        AwardRunSqlValue::Integer(i64::from(run.discord_message_sent)),
        text(run.status.as_str()),
        optional_text(&run.error_message),
        text(now),
        text(now),
    ]
}

pub fn award_run_update_bindings(run: &AwardRun, now: &str) -> Vec<AwardRunSqlValue> {
    vec![
        text(&run.period_type),
        optional_text(&run.winner_pubkey),
        optional_text(&run.winner_display_name),
        optional_text(&run.winner_name),
        optional_text(&run.winner_nip05),
        optional_text(&run.winner_picture),
        optional_datetime(run.latest_eligible_publication_at),
        optional_real(run.loops),
        optional_integer(run.views),
        optional_integer(run.unique_viewers),
        optional_integer(run.videos_with_views),
        optional_integer(run.positive_reactors),
        optional_integer(run.distinct_commenters),
        optional_integer(run.distinct_reposters),
        optional_integer(run.distinct_positive_engagers),
        optional_integer(run.engagement_tier),
        optional_real(run.engagement_rate),
        optional_real(run.score),
        optional_text(&run.award_event_id),
        optional_text(&run.prepared_award_event),
        AwardRunSqlValue::Integer(i64::from(run.discord_message_sent)),
        text(run.status.as_str()),
        optional_text(&run.error_message),
        text(now),
        text(&run.award_slug),
        text(&run.period_key),
    ]
}

pub fn claim_winner_bindings(run: &AwardRun, now: &str) -> Vec<AwardRunSqlValue> {
    vec![
        optional_text(&run.winner_pubkey),
        optional_text(&run.winner_display_name),
        optional_text(&run.winner_name),
        optional_text(&run.winner_nip05),
        optional_text(&run.winner_picture),
        optional_datetime(run.latest_eligible_publication_at),
        optional_real(run.loops),
        optional_integer(run.views),
        optional_integer(run.unique_viewers),
        optional_integer(run.videos_with_views),
        optional_integer(run.positive_reactors),
        optional_integer(run.distinct_commenters),
        optional_integer(run.distinct_reposters),
        optional_integer(run.distinct_positive_engagers),
        optional_integer(run.engagement_tier),
        optional_real(run.engagement_rate),
        optional_real(run.score),
        text(now),
        text(&run.award_slug),
        text(&run.period_key),
    ]
}

pub fn claim_prepared_award_bindings(
    award_slug: &str,
    period_key: &str,
    prepared_award_event: &str,
    award_event_id: &str,
    now: &str,
) -> Vec<AwardRunSqlValue> {
    vec![
        text(prepared_award_event),
        text(award_event_id),
        text(now),
        text(award_slug),
        text(period_key),
    ]
}

fn text(value: &str) -> AwardRunSqlValue {
    AwardRunSqlValue::Text(value.to_string())
}

fn optional_text(value: &Option<String>) -> AwardRunSqlValue {
    value.as_deref().map(text).unwrap_or(AwardRunSqlValue::Null)
}

fn optional_integer(value: Option<i64>) -> AwardRunSqlValue {
    value
        .map(AwardRunSqlValue::Integer)
        .unwrap_or(AwardRunSqlValue::Null)
}

fn optional_datetime(value: Option<chrono::DateTime<chrono::Utc>>) -> AwardRunSqlValue {
    value
        .map(|value| text(&value.to_rfc3339()))
        .unwrap_or(AwardRunSqlValue::Null)
}

fn optional_real(value: Option<f64>) -> AwardRunSqlValue {
    value
        .map(AwardRunSqlValue::Real)
        .unwrap_or(AwardRunSqlValue::Null)
}

#[cfg(target_arch = "wasm32")]
mod d1_repository {
    use async_trait::async_trait;
    use chrono::Utc;
    use serde::Deserialize;
    use wasm_bindgen::JsValue;
    use worker::D1Database;

    use crate::error::AppError;
    use crate::models::{AwardRun, BadgeDefinitionRecord};
    use crate::nostr::SignedNostrEvent;
    use crate::ports::AwardRepository;
    use crate::repository::AwardRunSqlValue;
    use crate::state::AwardRunStatus;

    #[derive(Debug)]
    pub struct D1AwardRepository {
        db: D1Database,
    }

    #[derive(Debug, Deserialize)]
    struct StoredBadgeDefinition {
        award_slug: String,
        d_tag: String,
        badge_name: String,
        description: String,
        image_url: String,
        thumb_url: String,
        definition_event_id: Option<String>,
        definition_coordinate: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct StoredAwardRun {
        award_slug: String,
        period_key: String,
        period_type: String,
        winner_pubkey: Option<String>,
        winner_display_name: Option<String>,
        winner_name: Option<String>,
        winner_nip05: Option<String>,
        winner_picture: Option<String>,
        latest_eligible_publication_at: Option<chrono::DateTime<chrono::Utc>>,
        loops: Option<f64>,
        views: Option<i64>,
        unique_viewers: Option<i64>,
        videos_with_views: Option<i64>,
        positive_reactors: Option<i64>,
        distinct_commenters: Option<i64>,
        distinct_reposters: Option<i64>,
        distinct_positive_engagers: Option<i64>,
        engagement_tier: Option<i64>,
        engagement_rate: Option<f64>,
        score: Option<f64>,
        award_event_id: Option<String>,
        prepared_award_event: Option<String>,
        discord_message_sent: i64,
        status: String,
        error_message: Option<String>,
    }

    impl D1AwardRepository {
        pub fn new(db: D1Database) -> Self {
            Self { db }
        }

        async fn load_run(
            &self,
            award_slug: &str,
            period_key: &str,
        ) -> Result<Option<AwardRun>, AppError> {
            let statement = self
                .db
                .prepare(crate::repository::award_run_by_key_sql())
                .bind(&[JsValue::from_str(award_slug), JsValue::from_str(period_key)])
                .map_err(repository_error)?;

            let row: Option<StoredAwardRun> =
                statement.first(None).await.map_err(repository_error)?;
            row.map(TryInto::try_into).transpose()
        }
    }

    #[async_trait(?Send)]
    impl AwardRepository for D1AwardRepository {
        async fn insert_badge_definition_seed(
            &self,
            record: &BadgeDefinitionRecord,
        ) -> Result<(), AppError> {
            let now = now_string();
            self.db
                .prepare(
                    "INSERT INTO badge_definitions (award_slug, d_tag, badge_name, description, image_url, thumb_url, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(award_slug) DO NOTHING",
                )
                .bind(&[
                    JsValue::from_str(&record.award_slug),
                    JsValue::from_str(&record.d_tag),
                    JsValue::from_str(&record.badge_name),
                    JsValue::from_str(&record.description),
                    JsValue::from_str(&record.image_url),
                    JsValue::from_str(&record.thumb_url),
                    JsValue::from_str(&now),
                    JsValue::from_str(&now),
                ])
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;
            Ok(())
        }

        async fn load_badge_definition(
            &self,
            award_slug: &str,
        ) -> Result<Option<BadgeDefinitionRecord>, AppError> {
            let statement = self
                .db
                .prepare(
                    "SELECT award_slug, d_tag, badge_name, description, image_url, thumb_url, definition_event_id, definition_coordinate FROM badge_definitions WHERE award_slug = ?1",
                )
                .bind(&[JsValue::from_str(award_slug)])
                .map_err(repository_error)?;

            let row: Option<StoredBadgeDefinition> =
                statement.first(None).await.map_err(repository_error)?;
            Ok(row.map(Into::into))
        }

        async fn save_badge_definition(
            &self,
            record: &BadgeDefinitionRecord,
        ) -> Result<(), AppError> {
            let now = now_string();
            self.db
                .prepare(crate::repository::save_badge_definition_sql())
                .bind(&[
                    option_string(&record.definition_event_id),
                    option_string(&record.definition_coordinate),
                    option_string(&Some(now.clone())),
                    JsValue::from_str(&now),
                    JsValue::from_str(&record.award_slug),
                ])
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;
            Ok(())
        }

        async fn upsert_award_run(&self, run: AwardRun) -> Result<AwardRun, AppError> {
            let now = now_string();
            let bindings =
                award_run_bindings_as_js(crate::repository::award_run_insert_bindings(&run, &now))?;
            self.db
                .prepare(crate::repository::upsert_award_run_sql())
                .bind(&bindings)
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(&run.award_slug, &run.period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing canonical award run".into()))
        }

        async fn save_award_run(&self, run: &AwardRun) -> Result<AwardRun, AppError> {
            let now = now_string();
            let bindings =
                award_run_bindings_as_js(crate::repository::award_run_update_bindings(run, &now))?;
            self.db
                .prepare(crate::repository::save_award_run_sql())
                .bind(&bindings)
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(&run.award_slug, &run.period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing saved award run".into()))
        }

        async fn claim_winner(&self, proposed: &AwardRun) -> Result<AwardRun, AppError> {
            let now = now_string();
            let bindings =
                award_run_bindings_as_js(crate::repository::claim_winner_bindings(proposed, &now))?;
            self.db
                .prepare(crate::repository::claim_winner_sql())
                .bind(&bindings)
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(&proposed.award_slug, &proposed.period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing claimed winner run".into()))
        }

        async fn claim_prepared_award(
            &self,
            award_slug: &str,
            period_key: &str,
            proposed: &SignedNostrEvent,
        ) -> Result<AwardRun, AppError> {
            let now = now_string();
            let serialized = serde_json::to_string(proposed)
                .map_err(|error| AppError::Repository(error.to_string()))?;
            let bindings =
                award_run_bindings_as_js(crate::repository::claim_prepared_award_bindings(
                    award_slug,
                    period_key,
                    &serialized,
                    &proposed.id,
                    &now,
                ))?;
            self.db
                .prepare(crate::repository::claim_prepared_award_sql())
                .bind(&bindings)
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(award_slug, period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing prepared award run".into()))
        }

        async fn load_recent_completed_runs(
            &self,
            award_slug: &str,
            limit: usize,
        ) -> Result<Vec<AwardRun>, AppError> {
            let statement = self
                .db
                .prepare(crate::repository::recent_completed_runs_sql())
                .bind(&[
                    JsValue::from_str(award_slug),
                    JsValue::from_f64(limit as f64),
                ])
                .map_err(repository_error)?;
            let rows: Vec<StoredAwardRun> = statement
                .all()
                .await
                .map_err(repository_error)?
                .results()
                .map_err(repository_error)?;
            rows.into_iter().map(TryInto::try_into).collect()
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
                None,
                None,
            )
            .await
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
                None,
                None,
            )
            .await
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
                None,
                None,
            )
            .await
        }

        async fn mark_awarded(
            &self,
            award_slug: &str,
            period_key: &str,
            award_event_id: &str,
        ) -> Result<AwardRun, AppError> {
            self.update_status(
                award_slug,
                period_key,
                AwardRunStatus::Awarded,
                None,
                Some(award_event_id),
                None,
            )
            .await
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
                None,
                None,
            )
            .await
        }

        async fn mark_completed(
            &self,
            award_slug: &str,
            period_key: &str,
        ) -> Result<AwardRun, AppError> {
            self.update_status(
                award_slug,
                period_key,
                AwardRunStatus::Completed,
                None,
                None,
                Some(true),
            )
            .await
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
                None,
                None,
            )
            .await
        }
    }

    impl D1AwardRepository {
        async fn update_status(
            &self,
            award_slug: &str,
            period_key: &str,
            status: AwardRunStatus,
            error_message: Option<&str>,
            award_event_id: Option<&str>,
            discord_message_sent: Option<bool>,
        ) -> Result<AwardRun, AppError> {
            let now = now_string();
            self.db
                .prepare(
                    "UPDATE award_runs SET award_event_id = COALESCE(?1, award_event_id), discord_message_sent = COALESCE(?2, discord_message_sent), status = ?3, error_message = ?4, updated_at = ?5 WHERE award_slug = ?6 AND period_key = ?7",
                )
                .bind(&[
                    option_string_ref(award_event_id),
                    option_bool_as_js(discord_message_sent),
                    JsValue::from_str(status.as_str()),
                    option_string_ref(error_message),
                    JsValue::from_str(&now),
                    JsValue::from_str(award_slug),
                    JsValue::from_str(period_key),
                ])
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(award_slug, period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing updated award run".into()))
        }
    }

    impl From<StoredBadgeDefinition> for BadgeDefinitionRecord {
        fn from(value: StoredBadgeDefinition) -> Self {
            Self {
                award_slug: value.award_slug,
                d_tag: value.d_tag,
                badge_name: value.badge_name,
                description: value.description,
                image_url: value.image_url,
                thumb_url: value.thumb_url,
                definition_event_id: value.definition_event_id,
                definition_coordinate: value.definition_coordinate,
            }
        }
    }

    impl TryFrom<StoredAwardRun> for AwardRun {
        type Error = AppError;

        fn try_from(value: StoredAwardRun) -> Result<Self, Self::Error> {
            Ok(Self {
                award_slug: value.award_slug,
                period_key: value.period_key,
                period_type: value.period_type,
                winner_pubkey: value.winner_pubkey,
                winner_display_name: value.winner_display_name,
                winner_name: value.winner_name,
                winner_nip05: value.winner_nip05,
                winner_picture: value.winner_picture,
                latest_eligible_publication_at: value.latest_eligible_publication_at,
                loops: value.loops,
                views: value.views,
                unique_viewers: value.unique_viewers,
                videos_with_views: value.videos_with_views,
                positive_reactors: value.positive_reactors,
                distinct_commenters: value.distinct_commenters,
                distinct_reposters: value.distinct_reposters,
                distinct_positive_engagers: value.distinct_positive_engagers,
                engagement_tier: value.engagement_tier,
                engagement_rate: value.engagement_rate,
                score: value.score,
                award_event_id: value.award_event_id,
                prepared_award_event: value.prepared_award_event,
                discord_message_sent: value.discord_message_sent != 0,
                status: AwardRunStatus::from_str(&value.status).ok_or_else(|| {
                    AppError::Repository(format!("unknown award run status {}", value.status))
                })?,
                error_message: value.error_message,
            })
        }
    }

    fn now_string() -> String {
        Utc::now().to_rfc3339()
    }

    fn option_string(value: &Option<String>) -> JsValue {
        value
            .as_ref()
            .map(|value| JsValue::from_str(value))
            .unwrap_or(JsValue::NULL)
    }

    fn option_string_ref(value: Option<&str>) -> JsValue {
        value.map(JsValue::from_str).unwrap_or(JsValue::NULL)
    }

    fn award_run_bindings_as_js(values: Vec<AwardRunSqlValue>) -> Result<Vec<JsValue>, AppError> {
        values
            .into_iter()
            .map(|value| match value {
                AwardRunSqlValue::Null => Ok(JsValue::NULL),
                AwardRunSqlValue::Text(value) => Ok(JsValue::from_str(&value)),
                AwardRunSqlValue::Real(value) => Ok(JsValue::from_f64(value)),
                AwardRunSqlValue::Integer(value) => {
                    if crate::repository::is_d1_safe_integer(value) {
                        Ok(JsValue::from_f64(value as f64))
                    } else {
                        Err(AppError::Repository(format!(
                            "integer value {value} cannot be represented exactly by a D1 JavaScript binding"
                        )))
                    }
                }
            })
            .collect()
    }

    fn bool_as_js(value: bool) -> JsValue {
        JsValue::from_f64(if value { 1.0 } else { 0.0 })
    }

    fn option_bool_as_js(value: Option<bool>) -> JsValue {
        value.map(bool_as_js).unwrap_or(JsValue::NULL)
    }

    fn repository_error(error: worker::Error) -> AppError {
        AppError::Repository(error.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use d1_repository::D1AwardRepository;
