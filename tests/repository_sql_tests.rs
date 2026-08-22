use divine_badges::models::AwardRun;
use divine_badges::repository::{
    award_run_by_key_sql, award_run_insert_bindings, award_run_unique_index_sql,
    award_run_update_bindings, claim_prepared_award_bindings, claim_prepared_award_sql,
    claim_winner_bindings, claim_winner_sql, is_d1_safe_integer, recent_completed_runs_sql,
    save_award_run_sql, save_badge_definition_sql, upsert_award_run_sql, AwardRunSqlValue,
};
use divine_badges::state::AwardRunStatus;

#[test]
fn pending_award_run_has_no_positive_engagement_score_receipt() {
    let run = AwardRun::pending("diviner-of-the-day", "2026-08-21", "day");

    assert_eq!(run.positive_reactors, None);
    assert_eq!(run.distinct_commenters, None);
    assert_eq!(run.distinct_reposters, None);
    assert_eq!(run.distinct_positive_engagers, None);
    assert_eq!(run.engagement_tier, None);
    assert_eq!(run.engagement_rate, None);
    assert_eq!(run.score, None);
    assert_eq!(run.latest_eligible_publication_at, None);
    assert_eq!(run.prepared_award_event, None);
}

#[test]
fn unique_index_targets_award_slug_and_period_key() {
    let sql = award_run_unique_index_sql();
    assert!(sql.contains("UNIQUE"));
    assert!(sql.contains("award_slug"));
    assert!(sql.contains("period_key"));
}

#[test]
fn badge_definition_save_statement_updates_publication_fields() {
    let sql = save_badge_definition_sql();
    assert!(sql.contains("badge_definitions"));
    assert!(sql.contains("definition_event_id"));
    assert!(sql.contains("definition_coordinate"));
}

#[test]
fn recent_completed_runs_query_filters_completed_rows() {
    let sql = recent_completed_runs_sql();
    assert!(sql.contains("WHERE award_slug = ?1"));
    assert!(sql.contains("status = 'completed'"));
    assert!(sql.contains("ORDER BY period_key DESC"));
    assert!(sql.contains("LIMIT ?2"));
}

#[test]
fn every_award_run_select_uses_the_complete_score_receipt_column_order() {
    let columns = "award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, latest_eligible_publication_at, loops, views, unique_viewers, videos_with_views, positive_reactors, distinct_commenters, distinct_reposters, distinct_positive_engagers, engagement_tier, engagement_rate, score, award_event_id, prepared_award_event, discord_message_sent, status, error_message";

    assert_eq!(
        award_run_by_key_sql(),
        format!("SELECT {columns} FROM award_runs WHERE award_slug = ?1 AND period_key = ?2")
    );
    assert_eq!(
        recent_completed_runs_sql(),
        format!(
            "SELECT {columns} FROM award_runs WHERE award_slug = ?1 AND status = 'completed' ORDER BY period_key DESC LIMIT ?2"
        )
    );
}

#[test]
fn award_run_insert_sql_and_bindings_share_the_complete_receipt_order() {
    let run = complete_award_run();

    assert_eq!(
        upsert_award_run_sql(),
        "INSERT INTO award_runs (award_slug, period_key, period_type, winner_pubkey, winner_display_name, winner_name, winner_nip05, winner_picture, latest_eligible_publication_at, loops, views, unique_viewers, videos_with_views, positive_reactors, distinct_commenters, distinct_reposters, distinct_positive_engagers, engagement_tier, engagement_rate, score, award_event_id, prepared_award_event, discord_message_sent, status, error_message, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27) ON CONFLICT(award_slug, period_key) DO NOTHING"
    );
    assert_eq!(
        award_run_insert_bindings(&run, "2026-08-22T00:00:00Z"),
        vec![
            AwardRunSqlValue::Text("diviner-of-the-day".into()),
            AwardRunSqlValue::Text("2026-08-21".into()),
            AwardRunSqlValue::Text("day".into()),
            AwardRunSqlValue::Text("winner-pubkey".into()),
            AwardRunSqlValue::Text("Winner".into()),
            AwardRunSqlValue::Text("winner".into()),
            AwardRunSqlValue::Text("winner@divine.video".into()),
            AwardRunSqlValue::Text("https://cdn.divine.video/winner.jpg".into()),
            AwardRunSqlValue::Text("2026-08-21T12:00:00+00:00".into()),
            AwardRunSqlValue::Real(12.5),
            AwardRunSqlValue::Integer(101),
            AwardRunSqlValue::Integer(91),
            AwardRunSqlValue::Integer(4),
            AwardRunSqlValue::Integer(13),
            AwardRunSqlValue::Integer(8),
            AwardRunSqlValue::Integer(5),
            AwardRunSqlValue::Integer(21),
            AwardRunSqlValue::Integer(1),
            AwardRunSqlValue::Real(0.230_769),
            AwardRunSqlValue::Real(88.75),
            AwardRunSqlValue::Text("award-event-id".into()),
            AwardRunSqlValue::Text("{\"id\":\"award-event-id\"}".into()),
            AwardRunSqlValue::Integer(1),
            AwardRunSqlValue::Text("completed".into()),
            AwardRunSqlValue::Text("receipt retained".into()),
            AwardRunSqlValue::Text("2026-08-22T00:00:00Z".into()),
            AwardRunSqlValue::Text("2026-08-22T00:00:00Z".into()),
        ]
    );
}

#[test]
fn full_winner_update_sql_and_bindings_share_the_complete_receipt_order() {
    let run = complete_award_run();

    assert_eq!(
        save_award_run_sql(),
        "UPDATE award_runs SET period_type = ?1, winner_pubkey = ?2, winner_display_name = ?3, winner_name = ?4, winner_nip05 = ?5, winner_picture = ?6, latest_eligible_publication_at = ?7, loops = ?8, views = ?9, unique_viewers = ?10, videos_with_views = ?11, positive_reactors = ?12, distinct_commenters = ?13, distinct_reposters = ?14, distinct_positive_engagers = ?15, engagement_tier = ?16, engagement_rate = ?17, score = ?18, award_event_id = ?19, prepared_award_event = ?20, discord_message_sent = ?21, status = ?22, error_message = ?23, updated_at = ?24 WHERE award_slug = ?25 AND period_key = ?26"
    );
    assert_eq!(
        award_run_update_bindings(&run, "2026-08-22T00:00:00Z"),
        vec![
            AwardRunSqlValue::Text("day".into()),
            AwardRunSqlValue::Text("winner-pubkey".into()),
            AwardRunSqlValue::Text("Winner".into()),
            AwardRunSqlValue::Text("winner".into()),
            AwardRunSqlValue::Text("winner@divine.video".into()),
            AwardRunSqlValue::Text("https://cdn.divine.video/winner.jpg".into()),
            AwardRunSqlValue::Text("2026-08-21T12:00:00+00:00".into()),
            AwardRunSqlValue::Real(12.5),
            AwardRunSqlValue::Integer(101),
            AwardRunSqlValue::Integer(91),
            AwardRunSqlValue::Integer(4),
            AwardRunSqlValue::Integer(13),
            AwardRunSqlValue::Integer(8),
            AwardRunSqlValue::Integer(5),
            AwardRunSqlValue::Integer(21),
            AwardRunSqlValue::Integer(1),
            AwardRunSqlValue::Real(0.230_769),
            AwardRunSqlValue::Real(88.75),
            AwardRunSqlValue::Text("award-event-id".into()),
            AwardRunSqlValue::Text("{\"id\":\"award-event-id\"}".into()),
            AwardRunSqlValue::Integer(1),
            AwardRunSqlValue::Text("completed".into()),
            AwardRunSqlValue::Text("receipt retained".into()),
            AwardRunSqlValue::Text("2026-08-22T00:00:00Z".into()),
            AwardRunSqlValue::Text("diviner-of-the-day".into()),
            AwardRunSqlValue::Text("2026-08-21".into()),
        ]
    );
}

#[test]
fn d1_integer_bindings_reject_values_javascript_cannot_represent_exactly() {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

    assert!(is_d1_safe_integer(MAX_SAFE_INTEGER));
    assert!(is_d1_safe_integer(-MAX_SAFE_INTEGER));
    assert!(!is_d1_safe_integer(MAX_SAFE_INTEGER + 1));
    assert!(!is_d1_safe_integer(-MAX_SAFE_INTEGER - 1));
}

#[test]
fn winner_nip05_migration_adds_nullable_column() {
    let sql = include_str!("../migrations/0003_winner_nip05.sql");
    assert!(sql.contains("ALTER TABLE award_runs"));
    assert!(sql.contains("winner_nip05"));
}

#[test]
fn positive_engagement_migration_adds_seven_nullable_receipt_columns() {
    let sql = include_str!("../migrations/0004_positive_engagement_scores.sql");
    let statements: Vec<_> = sql
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect();

    assert_eq!(
        statements,
        vec![
            "ALTER TABLE award_runs ADD COLUMN positive_reactors INTEGER",
            "ALTER TABLE award_runs ADD COLUMN distinct_commenters INTEGER",
            "ALTER TABLE award_runs ADD COLUMN distinct_reposters INTEGER",
            "ALTER TABLE award_runs ADD COLUMN distinct_positive_engagers INTEGER",
            "ALTER TABLE award_runs ADD COLUMN engagement_tier INTEGER",
            "ALTER TABLE award_runs ADD COLUMN engagement_rate REAL",
            "ALTER TABLE award_runs ADD COLUMN score REAL",
        ]
    );
}

#[test]
fn prepared_award_migration_adds_nullable_activity_and_signed_event_columns() {
    let sql = include_str!("../migrations/0005_prepared_award_event.sql");
    let statements: Vec<_> = sql
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect();

    assert_eq!(
        statements,
        vec![
            "ALTER TABLE award_runs ADD COLUMN latest_eligible_publication_at TEXT",
            "ALTER TABLE award_runs ADD COLUMN prepared_award_event TEXT",
        ]
    );
}

#[test]
fn winner_claim_is_atomic_and_cannot_overwrite_an_existing_receipt() {
    let run = complete_award_run();

    assert_eq!(
        claim_winner_sql(),
        "UPDATE award_runs SET winner_pubkey = ?1, winner_display_name = ?2, winner_name = ?3, winner_nip05 = ?4, winner_picture = ?5, latest_eligible_publication_at = ?6, loops = ?7, views = ?8, unique_viewers = ?9, videos_with_views = ?10, positive_reactors = ?11, distinct_commenters = ?12, distinct_reposters = ?13, distinct_positive_engagers = ?14, engagement_tier = ?15, engagement_rate = ?16, score = ?17, error_message = NULL, updated_at = ?18 WHERE award_slug = ?19 AND period_key = ?20 AND winner_pubkey IS NULL"
    );
    let bindings = claim_winner_bindings(&run, "2026-08-22T00:00:00Z");
    assert_eq!(bindings.len(), 20);
    assert_eq!(bindings[0], AwardRunSqlValue::Text("winner-pubkey".into()));
    assert_eq!(
        bindings[18],
        AwardRunSqlValue::Text("diviner-of-the-day".into())
    );
    assert_eq!(bindings[19], AwardRunSqlValue::Text("2026-08-21".into()));
}

#[test]
fn prepared_event_claim_is_atomic_and_sets_the_prepared_state_and_event_id() {
    assert_eq!(
        claim_prepared_award_sql(),
        "UPDATE award_runs SET prepared_award_event = ?1, award_event_id = ?2, status = 'award_prepared', error_message = NULL, updated_at = ?3 WHERE award_slug = ?4 AND period_key = ?5 AND prepared_award_event IS NULL"
    );
    assert_eq!(
        claim_prepared_award_bindings(
            "diviner-of-the-day",
            "2026-08-21",
            "{\"id\":\"event-id\"}",
            "event-id",
            "2026-08-22T00:00:00Z",
        ),
        vec![
            AwardRunSqlValue::Text("{\"id\":\"event-id\"}".into()),
            AwardRunSqlValue::Text("event-id".into()),
            AwardRunSqlValue::Text("2026-08-22T00:00:00Z".into()),
            AwardRunSqlValue::Text("diviner-of-the-day".into()),
            AwardRunSqlValue::Text("2026-08-21".into()),
        ]
    );
}

fn complete_award_run() -> AwardRun {
    let mut run = AwardRun::pending("diviner-of-the-day", "2026-08-21", "day");
    run.winner_pubkey = Some("winner-pubkey".into());
    run.winner_display_name = Some("Winner".into());
    run.winner_name = Some("winner".into());
    run.winner_nip05 = Some("winner@divine.video".into());
    run.winner_picture = Some("https://cdn.divine.video/winner.jpg".into());
    run.latest_eligible_publication_at = Some("2026-08-21T12:00:00Z".parse().unwrap());
    run.loops = Some(12.5);
    run.views = Some(101);
    run.unique_viewers = Some(91);
    run.videos_with_views = Some(4);
    run.positive_reactors = Some(13);
    run.distinct_commenters = Some(8);
    run.distinct_reposters = Some(5);
    run.distinct_positive_engagers = Some(21);
    run.engagement_tier = Some(1);
    run.engagement_rate = Some(0.230_769);
    run.score = Some(88.75);
    run.award_event_id = Some("award-event-id".into());
    run.prepared_award_event = Some("{\"id\":\"award-event-id\"}".into());
    run.discord_message_sent = true;
    run.status = AwardRunStatus::Completed;
    run.error_message = Some("receipt retained".into());
    run
}
