use chrono::{DateTime, Duration, Utc};

use crate::models::{CreatorLatestVideo, LeaderboardCreator};

pub fn is_active_creator(now: DateTime<Utc>, latest_video: &CreatorLatestVideo) -> bool {
    latest_video.published_at >= now - Duration::days(30)
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
        load_latest_video(&creator.pubkey)
            .map(|video| is_active_creator(now, &video))
            .unwrap_or(false)
    })
}
