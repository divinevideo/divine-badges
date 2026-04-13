use crate::models::{AwardRun, BadgeDefinitionRecord};

pub trait AwardRepository {
    fn load_badge_definition(
        &self,
        award_slug: &str,
    ) -> Result<Option<BadgeDefinitionRecord>, String>;
    fn save_badge_definition(&self, record: &BadgeDefinitionRecord) -> Result<(), String>;
    fn load_award_run(&self, award_slug: &str, period_key: &str) -> Result<Option<AwardRun>, String>;
}
