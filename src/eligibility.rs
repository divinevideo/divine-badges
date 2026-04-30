use chrono::{DateTime, Duration, Utc};

use crate::models::{CreatorLatestVideo, LeaderboardCreator};

// Rabble's founder account; Diviner awards should go to community creators after launch.
pub const DIVINER_AWARD_EXCLUDED_PUBKEYS: [&str; 1] =
    ["d95aa8fc0eff8e488952495b8064991d27fb96ed8652f12cdedc5a4e8b5ae540"];

pub fn is_active_creator(now: DateTime<Utc>, latest_video: &CreatorLatestVideo) -> bool {
    latest_video.published_at >= now - Duration::days(30)
}

pub fn is_diviner_award_excluded_creator(pubkey: &str) -> bool {
    DIVINER_AWARD_EXCLUDED_PUBKEYS
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(pubkey.trim()))
}

pub fn select_first_active_creator<'a, I, F>(
    now: DateTime<Utc>,
    ranked: I,
    mut load_latest_video: F,
) -> Option<&'a LeaderboardCreator>
where
    I: IntoIterator<Item = &'a LeaderboardCreator>,
    F: FnMut(&str) -> Option<CreatorLatestVideo>,
{
    ranked.into_iter().find(|creator| {
        if is_diviner_award_excluded_creator(&creator.pubkey) {
            return false;
        }

        load_latest_video(&creator.pubkey)
            .map(|video| is_active_creator(now, &video))
            .unwrap_or(false)
    })
}
