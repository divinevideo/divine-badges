use url::Url;

use crate::error::AppError;
use crate::models::{CreatorLatestVideo, LeaderboardCreator, LeaderboardResponse};

pub fn build_leaderboard_url(
    base_url: &str,
    period: &str,
    candidate_window: usize,
) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path("/api/leaderboard/creators");
    url.query_pairs_mut()
        .append_pair("period", period)
        .append_pair("limit", &candidate_window.to_string());
    Ok(url)
}

pub fn parse_leaderboard_response(body: &str) -> Result<LeaderboardResponse, AppError> {
    serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))
}

pub async fn ranked_creators_for_period(
    fetch_body: impl FnOnce(Url) -> Result<String, AppError>,
    base_url: &str,
    period: &str,
    candidate_window: usize,
) -> Result<Vec<LeaderboardCreator>, AppError> {
    let url = build_leaderboard_url(base_url, period, candidate_window)?;
    let body = fetch_body(url)?;
    let response = parse_leaderboard_response(&body)?;
    if response.entries.is_empty() {
        return Err(AppError::EmptyLeaderboard(period.to_string()));
    }
    Ok(response.entries)
}

pub fn build_latest_video_url(base_url: &str, pubkey: &str) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path(&format!("/api/users/{pubkey}/videos"));
    url.query_pairs_mut()
        .append_pair("sort", "published")
        .append_pair("limit", "1");
    Ok(url)
}

pub fn parse_latest_video_response(body: &str) -> Result<Option<CreatorLatestVideo>, AppError> {
    let parsed: serde_json::Value =
        serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))?;

    let first = parsed.as_array().and_then(|items| items.first()).cloned();

    match first {
        None => Ok(None),
        Some(value) => {
            let published_at = value
                .get("published_at")
                .and_then(|value| value.as_i64())
                .ok_or_else(|| AppError::Api("missing published_at".into()))?;

            let published_at = chrono::DateTime::<chrono::Utc>::from_timestamp(published_at, 0)
                .ok_or_else(|| AppError::Api("invalid published_at".into()))?;

            Ok(Some(CreatorLatestVideo { published_at }))
        }
    }
}
