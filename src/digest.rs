//! Builds the daily creator digest: what each active creator's videos earned
//! today, and the campaign that tells them.
//!
//! The counts come from pre-aggregated rollups, not per-video visibility
//! filtering: the per-video identity is already collapsed. The recipient gate
//! is what excludes removed or banned creators, and it is applied by the
//! endpoint that serves these rows. This module only renders the numbers.

use serde::Deserialize;

use crate::engagement::{AutomatedCampaign, PersonalizedRecipient};
use crate::error::AppError;
use crate::ports::CreatorPeriodStatsClient;

/// Page size requested from the endpoint. Ten full pages is the 5000-creator
/// audience cap.
pub const DIGEST_PAGE_LIMIT: usize = 500;

/// The one audience cap, in pages. Exceeding it fails; nothing truncates.
pub const DIGEST_MAX_PAGES: usize = 10;

/// One creator's engagement for a closed UTC period. Field names match the
/// endpoint's JSON, so deserialization needs no renaming.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreatorPeriodStats {
    pub pubkey: String,
    pub views: i64,
    pub unique_viewers: i64,
    pub loops: f64,
    pub reactions: i64,
    pub comments: i64,
    pub reposts: i64,
}

/// The endpoint response, reduced to the entries the digest needs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreatorPeriodStatsResponse {
    pub entries: Vec<CreatorPeriodStats>,
}

fn metric(count: i64, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}

/// Render a creator's day, or `None` when every metric is zero.
///
/// Only nonzero metrics appear, so a quiet day is not framed as a failure and
/// nobody is told they got nothing.
pub fn digest_body(stats: &CreatorPeriodStats) -> Option<String> {
    let mut parts = Vec::new();
    if stats.views > 0 {
        parts.push(metric(stats.views, "view", "views"));
    }
    if stats.reactions > 0 {
        parts.push(metric(stats.reactions, "like", "likes"));
    }
    if stats.comments > 0 {
        parts.push(metric(stats.comments, "comment", "comments"));
    }
    if stats.reposts > 0 {
        parts.push(metric(stats.reposts, "repost", "reposts"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("Your videos got {} today.", parts.join(" and ")))
}

/// Build the day's digest campaign, or `None` when no creator has activity.
pub fn digest_campaign(
    period_key: &str,
    stats: Vec<CreatorPeriodStats>,
) -> Option<AutomatedCampaign> {
    let personalized_recipients: Vec<PersonalizedRecipient> = stats
        .into_iter()
        .filter_map(|entry| {
            digest_body(&entry).map(|body| PersonalizedRecipient {
                pubkey: entry.pubkey,
                title: None,
                body,
            })
        })
        .collect();
    if personalized_recipients.is_empty() {
        return None;
    }

    Some(AutomatedCampaign {
        automation_key: format!("creator-digest-{period_key}"),
        name: format!("Creator digest {period_key}"),
        category: "engagement".to_string(),
        title: "Your day on Divine".to_string(),
        body: "Here is how your videos did today.".to_string(),
        tap_target_type: "app_route".to_string(),
        tap_target_value: "/me".to_string(),
        segment_type: "explicit_pubkey_list".to_string(),
        motivation: "Tell creators what their work earned today.".to_string(),
        success_metric: "Creator opens their analytics".to_string(),
        guardrail_metric: "Campaign opt-out rate".to_string(),
        expires_at: format!("{period_key}T23:59:59Z"),
        holdout_basis_points: 0,
        recipients: Vec::new(),
        personalized_recipients,
    })
}

/// Walk every creator with activity in the period, or fail.
///
/// Pages with `limit=500` from an empty cursor, sending the last entry's
/// pubkey as the next `after`. A page shorter than the limit is the true end,
/// because the endpoint applies the recipient gate and the zero-activity
/// `HAVING` before its own `LIMIT`. Any error propagates: a partial list must
/// never look complete. A full tenth page means the day exceeded the 5000
/// cap, and an arbitrary 5000 of them is the wrong answer.
pub async fn fetch_all_stats<C: CreatorPeriodStatsClient>(
    client: &C,
    period_key: &str,
) -> Result<Vec<CreatorPeriodStats>, AppError> {
    let mut collected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut after = String::new();
    let mut pages = 0usize;

    loop {
        let page = client
            .stats_page(period_key, DIGEST_PAGE_LIMIT, &after)
            .await?;
        let page_len = page.len();
        let next_after = page.last().map(|entry| entry.pubkey.clone());
        for entry in page {
            if seen.insert(entry.pubkey.clone()) {
                collected.push(entry);
            }
        }
        pages += 1;
        if page_len < DIGEST_PAGE_LIMIT {
            break;
        }
        if pages >= DIGEST_MAX_PAGES {
            return Err(AppError::Api(format!(
                "creator digest for {period_key} exceeded the {} creator audience cap",
                DIGEST_PAGE_LIMIT * DIGEST_MAX_PAGES
            )));
        }
        after = next_after.unwrap_or_default();
    }

    Ok(collected)
}
