pub fn award_run_unique_index_sql() -> &'static str {
    "CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx ON award_runs (award_slug, period_key);"
}

pub fn save_badge_definition_sql() -> &'static str {
    "UPDATE badge_definitions SET definition_event_id = ?1, definition_coordinate = ?2, published_at = ?3, updated_at = ?4 WHERE award_slug = ?5;"
}

pub fn recent_completed_runs_sql() -> &'static str {
    "SELECT award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, loops, views, unique_viewers, videos_with_views, award_event_id, discord_message_sent, status, error_message FROM award_runs WHERE award_slug = ?1 AND status = 'completed' ORDER BY period_key DESC LIMIT ?2"
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
    use crate::ports::AwardRepository;
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
        loops: Option<f64>,
        views: Option<i64>,
        unique_viewers: Option<i64>,
        videos_with_views: Option<i64>,
        award_event_id: Option<String>,
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
                .prepare(
                    "SELECT award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, loops, views, unique_viewers, videos_with_views, award_event_id, discord_message_sent, status, error_message FROM award_runs WHERE award_slug = ?1 AND period_key = ?2",
                )
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
            self.db
                .prepare(
                    "INSERT INTO award_runs (award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, loops, views, unique_viewers, videos_with_views, award_event_id, discord_message_sent, status, error_message, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18) ON CONFLICT(award_slug, period_key) DO NOTHING",
                )
                .bind(&[
                    JsValue::from_str(&run.award_slug),
                    JsValue::from_str(&run.period_key),
                    JsValue::from_str(&run.period_type),
                    option_string(&run.winner_pubkey),
                    option_string(&run.winner_display_name),
                    option_string(&run.winner_name),
                    option_string(&run.winner_nip05),
                    option_string(&run.winner_picture),
                    option_f64(run.loops),
                    option_i64(run.views),
                    option_i64(run.unique_viewers),
                    option_i64(run.videos_with_views),
                    option_string(&run.award_event_id),
                    bool_as_js(run.discord_message_sent),
                    JsValue::from_str(run.status.as_str()),
                    option_string(&run.error_message),
                    JsValue::from_str(&now),
                    JsValue::from_str(&now),
                ])
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
            self.db
                .prepare(
                    "UPDATE award_runs SET period_type = ?1, winner_pubkey = ?2, winner_display_name = ?3, winner_name = ?4, winner_nip05 = ?5, winner_picture = ?6, loops = ?7, views = ?8, unique_viewers = ?9, videos_with_views = ?10, award_event_id = ?11, discord_message_sent = ?12, status = ?13, error_message = ?14, updated_at = ?15 WHERE award_slug = ?16 AND period_key = ?17",
                )
                .bind(&[
                    JsValue::from_str(&run.period_type),
                    option_string(&run.winner_pubkey),
                    option_string(&run.winner_display_name),
                    option_string(&run.winner_name),
                    option_string(&run.winner_nip05),
                    option_string(&run.winner_picture),
                    option_f64(run.loops),
                    option_i64(run.views),
                    option_i64(run.unique_viewers),
                    option_i64(run.videos_with_views),
                    option_string(&run.award_event_id),
                    bool_as_js(run.discord_message_sent),
                    JsValue::from_str(run.status.as_str()),
                    option_string(&run.error_message),
                    JsValue::from_str(&now),
                    JsValue::from_str(&run.award_slug),
                    JsValue::from_str(&run.period_key),
                ])
                .map_err(repository_error)?
                .run()
                .await
                .map_err(repository_error)?;

            self.load_run(&run.award_slug, &run.period_key)
                .await?
                .ok_or_else(|| AppError::Repository("missing saved award run".into()))
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
                loops: value.loops,
                views: value.views,
                unique_viewers: value.unique_viewers,
                videos_with_views: value.videos_with_views,
                award_event_id: value.award_event_id,
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

    fn option_f64(value: Option<f64>) -> JsValue {
        value.map(JsValue::from_f64).unwrap_or(JsValue::NULL)
    }

    fn option_i64(value: Option<i64>) -> JsValue {
        value
            .map(|value| JsValue::from_f64(value as f64))
            .unwrap_or(JsValue::NULL)
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
