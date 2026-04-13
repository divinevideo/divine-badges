use divine_badges::repository::{award_run_unique_index_sql, save_badge_definition_sql};

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
