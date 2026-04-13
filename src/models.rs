use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::awards::AwardDefinition;
use crate::state::AwardRunStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct AwardRun {
    pub award_slug: String,
    pub period_key: String,
    pub period_type: String,
    pub winner_pubkey: Option<String>,
    pub winner_display_name: Option<String>,
    pub winner_name: Option<String>,
    pub winner_picture: Option<String>,
    pub loops: Option<f64>,
    pub views: Option<i64>,
    pub unique_viewers: Option<i64>,
    pub videos_with_views: Option<i64>,
    pub award_event_id: Option<String>,
    pub discord_message_sent: bool,
    pub status: AwardRunStatus,
    pub error_message: Option<String>,
}

impl AwardRun {
    pub fn pending(award_slug: &str, period_key: &str, period_type: &str) -> Self {
        Self {
            award_slug: award_slug.to_string(),
            period_key: period_key.to_string(),
            period_type: period_type.to_string(),
            winner_pubkey: None,
            winner_display_name: None,
            winner_name: None,
            winner_picture: None,
            loops: None,
            views: None,
            unique_viewers: None,
            videos_with_views: None,
            award_event_id: None,
            discord_message_sent: false,
            status: AwardRunStatus::Pending,
            error_message: None,
        }
    }

    pub fn completed(
        award_slug: &str,
        period_key: &str,
        period_type: &str,
        award_event_id: &str,
    ) -> Self {
        let mut run = Self::pending(award_slug, period_key, period_type);
        run.status = AwardRunStatus::Completed;
        run.discord_message_sent = true;
        run.award_event_id = Some(award_event_id.to_string());
        run
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BadgeDefinitionRecord {
    pub award_slug: String,
    pub d_tag: String,
    pub badge_name: String,
    pub description: String,
    pub image_url: String,
    pub thumb_url: String,
    pub definition_event_id: Option<String>,
    pub definition_coordinate: Option<String>,
}

impl BadgeDefinitionRecord {
    pub fn from_award(award: &AwardDefinition, image_url: &str) -> Self {
        Self {
            award_slug: award.slug.to_string(),
            d_tag: award.d_tag.to_string(),
            badge_name: award.badge_name.to_string(),
            description: award.description.to_string(),
            image_url: image_url.to_string(),
            thumb_url: image_url.to_string(),
            definition_event_id: None,
            definition_coordinate: None,
        }
    }

    pub fn published(
        award: &AwardDefinition,
        image_url: &str,
        definition_event_id: &str,
        definition_coordinate: &str,
    ) -> Self {
        let mut record = Self::from_award(award, image_url);
        record.definition_event_id = Some(definition_event_id.to_string());
        record.definition_coordinate = Some(definition_coordinate.to_string());
        record
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardResponse {
    pub period: String,
    pub entries: Vec<LeaderboardCreator>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardCreator {
    pub pubkey: String,
    pub display_name: String,
    pub name: String,
    pub picture: String,
    pub loops: f64,
    pub views: i64,
    pub unique_viewers: i64,
    pub videos_with_views: i64,
}

impl LeaderboardCreator {
    pub fn best_display_name(&self) -> String {
        if !self.display_name.trim().is_empty() {
            self.display_name.clone()
        } else if !self.name.trim().is_empty() {
            self.name.clone()
        } else {
            self.pubkey.chars().take(8).collect()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatorLatestVideo {
    pub published_at: DateTime<Utc>,
}
