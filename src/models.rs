use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct AwardRun {
    pub award_slug: String,
    pub period_key: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BadgeDefinitionRecord {
    pub award_slug: String,
    pub d_tag: String,
    pub definition_event_id: Option<String>,
    pub definition_coordinate: Option<String>,
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
