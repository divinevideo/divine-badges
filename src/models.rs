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
