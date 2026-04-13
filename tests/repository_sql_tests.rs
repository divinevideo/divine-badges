use divine_badges::repository::{
    award_run_unique_index_sql, recent_completed_runs_sql, save_badge_definition_sql,
};

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
fn winner_nip05_migration_adds_nullable_column() {
    let sql = include_str!("../migrations/0003_winner_nip05.sql");
    assert!(sql.contains("ALTER TABLE award_runs"));
    assert!(sql.contains("winner_nip05"));
}
