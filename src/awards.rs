#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AwardDefinition {
    pub slug: &'static str,
    pub d_tag: &'static str,
    pub badge_name: &'static str,
    pub description: &'static str,
}

pub fn award_catalog() -> Vec<AwardDefinition> {
    vec![
        AwardDefinition {
            slug: "diviner_of_the_day",
            d_tag: "diviner-of-the-day",
            badge_name: "Diviner of the Day",
            description: "Awarded to the top Divine creator of the day across all videos.",
        },
        AwardDefinition {
            slug: "diviner_of_the_week",
            d_tag: "diviner-of-the-week",
            badge_name: "Diviner of the Week",
            description: "Awarded to the top Divine creator of the week across all videos.",
        },
        AwardDefinition {
            slug: "diviner_of_the_month",
            d_tag: "diviner-of-the-month",
            badge_name: "Diviner of the Month",
            description: "Awarded to the top Divine creator of the month across all videos.",
        },
    ]
}

pub fn award_for_period_kind(kind: &str) -> Option<AwardDefinition> {
    match kind {
        "day" => award_catalog()
            .into_iter()
            .find(|award| award.slug == "diviner_of_the_day"),
        "week" => award_catalog()
            .into_iter()
            .find(|award| award.slug == "diviner_of_the_week"),
        "month" => award_catalog()
            .into_iter()
            .find(|award| award.slug == "diviner_of_the_month"),
        _ => None,
    }
}
