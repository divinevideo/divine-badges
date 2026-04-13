pub struct AwardDefinition {
    pub slug: &'static str,
}

pub fn award_catalog() -> Vec<AwardDefinition> {
    vec![
        AwardDefinition {
            slug: "diviner_of_the_day",
        },
        AwardDefinition {
            slug: "diviner_of_the_week",
        },
        AwardDefinition {
            slug: "diviner_of_the_month",
        },
    ]
}
