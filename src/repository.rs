pub fn award_run_unique_index_sql() -> &'static str {
    "CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx ON award_runs (award_slug, period_key);"
}

pub fn save_badge_definition_sql() -> &'static str {
    "UPDATE badge_definitions SET definition_event_id = ?1, definition_coordinate = ?2, published_at = ?3, updated_at = ?4 WHERE award_slug = ?5;"
}
