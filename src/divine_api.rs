use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use k256::schnorr::VerifyingKey;
use url::Url;

use crate::digest::CreatorPeriodStatsResponse;
use crate::error::AppError;
use crate::models::{DivinerCandidate, DivinerCandidatesResponse};

const MIN_CANDIDATE_WINDOW: usize = 1;
const MAX_CANDIDATE_WINDOW: usize = 100;

fn canonical_utc_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn validate_candidate_window(candidate_window: usize) -> Result<(), AppError> {
    if !(MIN_CANDIDATE_WINDOW..=MAX_CANDIDATE_WINDOW).contains(&candidate_window) {
        return Err(AppError::Api(format!(
            "Diviner candidate window must be between {MIN_CANDIDATE_WINDOW} and {MAX_CANDIDATE_WINDOW}"
        )));
    }
    Ok(())
}

pub fn build_diviner_candidates_url(
    base_url: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    candidate_window: usize,
) -> Result<Url, AppError> {
    validate_candidate_window(candidate_window)?;
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

pub fn validate_diviner_candidates_response(
    response: DivinerCandidatesResponse,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    candidate_window: usize,
) -> Result<Vec<DivinerCandidate>, AppError> {
    validate_candidate_window(candidate_window)?;

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
        return Err(AppError::EmptyCandidates(format!(
            "{}..{}",
            canonical_utc_timestamp(start),
            canonical_utc_timestamp(end)
        )));
    }
    if response.entries.len() > candidate_window {
        return Err(AppError::Api(format!(
            "Diviner candidates response returned {} entries for requested maximum {candidate_window}",
            response.entries.len()
        )));
    }

    for (index, candidate) in response.entries.iter().enumerate() {
        let position = index + 1;
        let expected_rank = position as u64;
        let candidate_error = |message: &str| {
            AppError::Api(format!(
                "invalid Diviner candidate at position {position} (reported rank {}): {message}",
                candidate.rank
            ))
        };

        if candidate.rank != expected_rank {
            return Err(candidate_error(&format!(
                "expected rank {expected_rank}, reported rank {}",
                candidate.rank
            )));
        }
        if candidate.pubkey.len() != 64
            || !candidate
                .pubkey
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(candidate_error(
                "pubkey must contain exactly 64 ASCII hexadecimal characters",
            ));
        }
        let pubkey_bytes = hex::decode(&candidate.pubkey)
            .map_err(|_| candidate_error("pubkey hexadecimal decoding failed"))?;
        if VerifyingKey::from_bytes(&pubkey_bytes).is_err() {
            return Err(candidate_error(
                "pubkey must be a valid secp256k1 x-only verifying key",
            ));
        }
        if !candidate.loops.is_finite() || candidate.loops < 0.0 {
            return Err(candidate_error("loops must be finite and nonnegative"));
        }
        if !candidate.engagement_rate.is_finite() || candidate.engagement_rate < 0.0 {
            return Err(candidate_error(
                "engagement_rate must be finite and nonnegative",
            ));
        }
        if !candidate.score.is_finite() || !(0.0..=100.0).contains(&candidate.score) {
            return Err(candidate_error(
                "score must be finite and in the range 0 through 100",
            ));
        }
        if !matches!(candidate.engagement_tier, 0 | 1) {
            return Err(candidate_error("engagement_tier must be 0 or 1"));
        }
    }

    Ok(response.entries)
}

fn candidates_from_http_response(
    status_code: u16,
    body: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    candidate_window: usize,
) -> Result<Vec<DivinerCandidate>, AppError> {
    ensure_successful_candidates_status(status_code)?;

    let response = parse_diviner_candidates_response(body)?;
    validate_diviner_candidates_response(response, start, end, candidate_window)
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
    candidates_from_http_response(status_code, &body, start, end, candidate_window)
}

/// The exact, end-exclusive UTC bounds of one closed daily period key.
fn creator_period_bounds(period_key: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let date = NaiveDate::parse_from_str(period_key, "%F")
        .map_err(|err| AppError::Api(format!("invalid creator period key {period_key}: {err}")))?;
    let start = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Api(format!("invalid creator period start {period_key}")))?
        .and_utc();
    let end = start + chrono::Duration::days(1);
    Ok((start, end))
}

pub fn build_creator_period_stats_url(
    base_url: &str,
    period_key: &str,
    limit: usize,
    after: &str,
) -> Result<Url, AppError> {
    let (start, end) = creator_period_bounds(period_key)?;
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path("/api/awards/creator-period-stats");
    url.query_pairs_mut()
        .append_pair("start", &canonical_utc_timestamp(start))
        .append_pair("end", &canonical_utc_timestamp(end))
        .append_pair("limit", &limit.to_string());
    if !after.is_empty() {
        url.query_pairs_mut().append_pair("after", after);
    }
    Ok(url)
}

/// Parse one stats page. `next_after` must be present, even as `null`: serde
/// would read a missing field as `None`, and a response without the field
/// would then end the walk after one page and look complete.
pub fn parse_creator_period_stats_response(
    body: &str,
) -> Result<CreatorPeriodStatsResponse, AppError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))?;
    if value.get("next_after").is_none() {
        return Err(AppError::Api(
            "creator period stats response has no next_after".into(),
        ));
    }
    serde_json::from_value(value).map_err(|err| AppError::Api(err.to_string()))
}

#[cfg(target_arch = "wasm32")]
mod wasm_clients {
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use worker::Fetch;

    use crate::error::AppError;
    use crate::models::DivinerCandidate;
    use crate::ports::{CreatorPeriodStatsClient, DivinerCandidatesClient};

    use super::{
        build_creator_period_stats_url, build_diviner_candidates_url,
        candidates_from_http_response, ensure_successful_candidates_status,
        parse_creator_period_stats_response,
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

            candidates_from_http_response(status_code, &body, start, end, candidate_window)
        }
    }

    /// Reads the paged per-creator stats endpoint that feeds the daily digest.
    #[derive(Debug, Clone)]
    pub struct WasmCreatorPeriodStatsClient {
        base_url: String,
    }

    impl WasmCreatorPeriodStatsClient {
        pub fn new(base_url: String) -> Self {
            Self { base_url }
        }
    }

    #[async_trait(?Send)]
    impl CreatorPeriodStatsClient for WasmCreatorPeriodStatsClient {
        async fn stats_page(
            &self,
            period_key: &str,
            limit: usize,
            after: &str,
        ) -> Result<crate::digest::CreatorPeriodStatsResponse, AppError> {
            let url = build_creator_period_stats_url(&self.base_url, period_key, limit, after)?;
            let mut response = Fetch::Url(url)
                .send()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            let status_code = response.status_code();
            if !(200..300).contains(&status_code) {
                return Err(AppError::Api(format!(
                    "creator period stats request failed with {status_code}"
                )));
            }
            let body = response
                .text()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            parse_creator_period_stats_response(&body)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_clients::{WasmCreatorPeriodStatsClient, WasmDivinerCandidatesClient};
