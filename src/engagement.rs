//! Builds the campaigns divine-engagement creates when an award completes.
//!
//! This module decides only what to say and to whom. Consent, quiet hours,
//! caps, and device validity are divine-push-service's decisions, and the
//! delivery gate and global pause remain divine-engagement's.

use chrono::{Duration, NaiveDate};
use serde::Serialize;

use crate::models::AwardRun;

/// The request body for `POST /api/internal/campaigns`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomatedCampaign {
    pub automation_key: String,
    pub name: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub tap_target_type: String,
    pub tap_target_value: String,
    pub segment_type: String,
    pub motivation: String,
    pub success_metric: String,
    pub guardrail_metric: String,
    pub expires_at: String,
    pub holdout_basis_points: u32,
    pub recipients: Vec<String>,
}

/// The winner's own name, or neutral copy when the profile has none.
fn winner_label(run: &AwardRun) -> &str {
    run.winner_display_name
        .as_deref()
        .or(run.winner_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Today's Diviner")
}

/// A campaign expires at the end of the UTC day it announces: a Diviner
/// notification arriving two days late is worse than one that never arrives.
///
/// The award for day D is only decided once D has closed, so the campaign
/// is created and announced on D+1 and expires when D+1 ends. Expiring at the
/// end of D itself would make every campaign expired on arrival.
///
/// Only the daily award has campaigns: the copy and keys speak of "today",
/// and a week or month key is not a date.
fn expires_at(run: &AwardRun) -> Option<String> {
    if run.period_type != "day" {
        return None;
    }
    let day = NaiveDate::parse_from_str(&run.period_key, "%F").ok()?;
    let announce_day_end = day.checked_add_signed(Duration::days(2))?;
    Some(format!("{}T00:00:00Z", announce_day_end.format("%F")))
}

/// Tell the winner they won.
pub fn winner_campaign(run: &AwardRun) -> Option<AutomatedCampaign> {
    let winner = run.winner_pubkey.as_deref()?;
    let expires_at = expires_at(run)?;

    Some(AutomatedCampaign {
        automation_key: format!("diviner-day-{}-winner", run.period_key),
        name: format!("Diviner of the Day {} — winner", run.period_key),
        category: "engagement".to_string(),
        title: "You are today's Diviner".to_string(),
        body: "You are today's Diviner. Your badge is waiting, and people are on their way to your profile.".to_string(),
        tap_target_type: "app_route".to_string(),
        tap_target_value: format!("/profile/{winner}"),
        segment_type: "explicit_pubkey_list".to_string(),
        motivation: "Tell the person who won that they won.".to_string(),
        success_metric: "Winner opens their badge".to_string(),
        guardrail_metric: "Campaign opt-out rate".to_string(),
        expires_at,
        holdout_basis_points: 0,
        recipients: vec![winner.to_string()],
    })
}

/// Send everyone else to the winner's profile.
pub fn broadcast_campaign(run: &AwardRun) -> Option<AutomatedCampaign> {
    let winner = run.winner_pubkey.as_deref()?;
    let expires_at = expires_at(run)?;
    let label = winner_label(run);

    Some(AutomatedCampaign {
        automation_key: format!("diviner-day-{}-broadcast", run.period_key),
        name: format!("Diviner of the Day {} — broadcast", run.period_key),
        category: "engagement".to_string(),
        title: "Diviner of the Day".to_string(),
        body: format!("{label} is today's Diviner. Go see what they made."),
        tap_target_type: "app_route".to_string(),
        tap_target_value: format!("/profile/{winner}"),
        segment_type: "opted_in_push_audience".to_string(),
        motivation:
            "Send the day's winner an audience, and give everyone else a reason to open the app."
                .to_string(),
        success_metric: "Diviner profile visits".to_string(),
        guardrail_metric: "Campaign opt-out rate".to_string(),
        expires_at,
        holdout_basis_points: 0,
        recipients: Vec::new(),
    })
}

#[cfg(target_arch = "wasm32")]
mod wasm_client {
    use async_trait::async_trait;
    use wasm_bindgen::JsValue;
    use worker::{Fetch, Headers, Method, Request, RequestInit};

    use super::AutomatedCampaign;
    use crate::error::AppError;
    use crate::ports::CampaignClient;

    #[derive(Debug, Clone)]
    pub struct WasmCampaignClient {
        base_url: String,
        access_client_id: String,
        access_client_secret: String,
    }

    impl WasmCampaignClient {
        pub fn new(
            base_url: String,
            access_client_id: String,
            access_client_secret: String,
        ) -> Self {
            Self {
                base_url,
                access_client_id,
                access_client_secret,
            }
        }
    }

    #[async_trait(?Send)]
    impl CampaignClient for WasmCampaignClient {
        async fn create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError> {
            let body =
                serde_json::to_string(campaign).map_err(|err| AppError::Api(err.to_string()))?;
            let url = format!(
                "{}/api/internal/campaigns",
                self.base_url.trim_end_matches('/')
            );

            let mut init = RequestInit::new();
            init.with_method(Method::Post);
            init.with_body(Some(JsValue::from_str(&body)));

            let headers = Headers::new();
            headers
                .set("Content-Type", "application/json")
                .map_err(|err| AppError::Api(err.to_string()))?;
            headers
                .set("CF-Access-Client-Id", &self.access_client_id)
                .map_err(|err| AppError::Api(err.to_string()))?;
            headers
                .set("CF-Access-Client-Secret", &self.access_client_secret)
                .map_err(|err| AppError::Api(err.to_string()))?;
            init.with_headers(headers);

            let request = Request::new_with_init(&url, &init)
                .map_err(|err| AppError::Api(err.to_string()))?;
            let response = Fetch::Request(request)
                .send()
                .await
                .map_err(|err| AppError::Api(err.to_string()))?;
            let status = response.status_code();
            if !(200..300).contains(&status) {
                return Err(AppError::Api(format!(
                    "engagement campaign request failed with {status}"
                )));
            }
            Ok(())
        }
    }

    /// A client that never sends. Used when the engagement API is unconfigured.
    #[derive(Debug, Clone, Default)]
    pub struct NoopCampaignClient;

    #[async_trait(?Send)]
    impl CampaignClient for NoopCampaignClient {
        async fn create_campaign(&self, _campaign: &AutomatedCampaign) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// The concrete campaign client the Worker runs with: active when the
    /// engagement API and its Access credentials are configured, a no-op
    /// otherwise.
    #[derive(Debug, Clone)]
    pub enum EngagementCampaignClient {
        Active(Box<WasmCampaignClient>),
        Disabled(NoopCampaignClient),
    }

    impl EngagementCampaignClient {
        pub fn from_config(config: &crate::config::AppConfig) -> Self {
            match config.engagement_api() {
                Some((base_url, client_id, client_secret)) => {
                    Self::Active(Box::new(WasmCampaignClient::new(
                        base_url.to_string(),
                        client_id.to_string(),
                        client_secret.to_string(),
                    )))
                }
                None => Self::Disabled(NoopCampaignClient),
            }
        }
    }

    #[async_trait(?Send)]
    impl CampaignClient for EngagementCampaignClient {
        async fn create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError> {
            match self {
                Self::Active(client) => client.create_campaign(campaign).await,
                Self::Disabled(client) => client.create_campaign(campaign).await,
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_client::{EngagementCampaignClient, NoopCampaignClient, WasmCampaignClient};
