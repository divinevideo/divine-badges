use chrono::{DateTime, SecondsFormat, Utc};
use url::Url;

use crate::error::AppError;
use crate::models::{
    CreatorLatestVideo, DivinerCandidate, DivinerCandidatesResponse, LeaderboardCreator,
    LeaderboardResponse,
};

fn canonical_utc_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn build_diviner_candidates_url(
    base_url: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    candidate_window: usize,
) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path("/api/awards/diviner-candidates");
    url.query_pairs_mut()
        .append_pair("start", &canonical_utc_timestamp(start))
        .append_pair("end", &canonical_utc_timestamp(end))
        .append_pair("limit", &candidate_window.to_string());
    Ok(url)
}

pub fn parse_diviner_candidates_response(
    body: &str,
) -> Result<DivinerCandidatesResponse, AppError> {
    serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))
}

fn candidates_from_http_response(
    status_code: u16,
    body: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<DivinerCandidate>, AppError> {
    ensure_successful_candidates_status(status_code)?;

    let response = parse_diviner_candidates_response(body)?;
    if response.start != start || response.end != end {
        return Err(AppError::Api(format!(
            "Diviner candidates period mismatch: expected {}..{}, received {}..{}",
            canonical_utc_timestamp(start),
            canonical_utc_timestamp(end),
            canonical_utc_timestamp(response.start),
            canonical_utc_timestamp(response.end)
        )));
    }
    if response.entries.is_empty() {
        return Err(AppError::EmptyLeaderboard(format!(
            "{}..{}",
            canonical_utc_timestamp(start),
            canonical_utc_timestamp(end)
        )));
    }
    Ok(response.entries)
}

fn ensure_successful_candidates_status(status_code: u16) -> Result<(), AppError> {
    if !(200..300).contains(&status_code) {
        return Err(AppError::Api(format!(
            "Diviner candidates request failed with {status_code}"
        )));
    }
    Ok(())
}

pub async fn ranked_candidates_for_period(
    fetch_response: impl FnOnce(Url) -> Result<(u16, String), AppError>,
    base_url: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    candidate_window: usize,
) -> Result<Vec<DivinerCandidate>, AppError> {
    let url = build_diviner_candidates_url(base_url, start, end, candidate_window)?;
    let (status_code, body) = fetch_response(url)?;
    candidates_from_http_response(status_code, &body, start, end)
}

// Temporary compatibility helpers for rolling leaderboard call sites. Remove
// with the exact-boundary selection migration in Task 11.

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

#[cfg(target_arch = "wasm32")]
mod wasm_clients {
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use worker::Fetch;

    use crate::error::AppError;
    use crate::models::{CreatorLatestVideo, DivinerCandidate, LeaderboardCreator};
    use crate::ports::{CreatorActivityClient, DivinerCandidatesClient, LeaderboardClient};

    use super::{
        build_diviner_candidates_url, build_latest_video_url, build_leaderboard_url,
        candidates_from_http_response, ensure_successful_candidates_status,
        parse_latest_video_response, parse_leaderboard_response,
    };

    #[derive(Debug, Clone)]
    pub struct WasmDivinerCandidatesClient {
        base_url: String,
    }

    impl WasmDivinerCandidatesClient {
        pub fn new(base_url: String) -> Self {
            Self { base_url }
        }
    }

    #[async_trait(?Send)]
    impl DivinerCandidatesClient for WasmDivinerCandidatesClient {
        async fn ranked_candidates(
            &self,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
            candidate_window: usize,
        ) -> Result<Vec<DivinerCandidate>, AppError> {
            let url = build_diviner_candidates_url(&self.base_url, start, end, candidate_window)?;
            let mut response = Fetch::Url(url)
                .send()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            let status_code = response.status_code();
            ensure_successful_candidates_status(status_code)?;
            let body = response
                .text()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;

            candidates_from_http_response(status_code, &body, start, end)
        }
    }

    /// Temporary compatibility adapter for rolling leaderboard call sites.
    /// Remove with the exact-boundary selection migration in Task 11.
    #[derive(Debug, Clone)]
    pub struct WasmLeaderboardClient {
        base_url: String,
    }

    impl WasmLeaderboardClient {
        pub fn new(base_url: String) -> Self {
            Self { base_url }
        }
    }

    #[async_trait(?Send)]
    impl LeaderboardClient for WasmLeaderboardClient {
        async fn ranked_creators(
            &self,
            period: &str,
            candidate_window: usize,
        ) -> Result<Vec<LeaderboardCreator>, AppError> {
            let url = build_leaderboard_url(&self.base_url, period, candidate_window)?;
            let mut response = Fetch::Url(url)
                .send()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            if !(200..300).contains(&response.status_code()) {
                return Err(AppError::Api(format!(
                    "leaderboard request failed with {}",
                    response.status_code()
                )));
            }

            let body = response
                .text()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            Ok(parse_leaderboard_response(&body)?.entries)
        }
    }

    #[derive(Debug, Clone)]
    pub struct WasmActivityClient {
        base_url: String,
    }

    impl WasmActivityClient {
        pub fn new(base_url: String) -> Self {
            Self { base_url }
        }
    }

    #[async_trait(?Send)]
    impl CreatorActivityClient for WasmActivityClient {
        async fn latest_video(&self, pubkey: &str) -> Result<Option<CreatorLatestVideo>, AppError> {
            let url = build_latest_video_url(&self.base_url, pubkey)?;
            let mut response = Fetch::Url(url)
                .send()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            if !(200..300).contains(&response.status_code()) {
                return Err(AppError::Api(format!(
                    "latest video request failed with {}",
                    response.status_code()
                )));
            }

            let body = response
                .text()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            parse_latest_video_response(&body)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_clients::{WasmActivityClient, WasmDivinerCandidatesClient, WasmLeaderboardClient};
