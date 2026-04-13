pub fn build_announcement_message(
    award_name: &str,
    winner_name: &str,
    loops: f64,
    creator_link: &str,
) -> String {
    format!(
        "{award_name}: {winner_name} won with {} loops. {creator_link}",
        loops.round() as i64
    )
}
