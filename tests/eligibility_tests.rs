use chrono::{TimeZone, Utc};
use divine_badges::eligibility::{is_active_creator, select_first_active_creator};
use divine_badges::models::{CreatorLatestVideo, DivinerCandidate};

fn candidate(pubkey: &str, display_name: &str, rank: u64) -> DivinerCandidate {
    DivinerCandidate {
        pubkey: pubkey.into(),
        display_name: display_name.into(),
        name: display_name.into(),
        nip05: None,
        picture: String::new(),
        views: 1,
        unique_viewers: 1,
        loops: 1.0,
        videos_with_views: 1,
        positive_reactors: 1,
        distinct_commenters: 1,
        distinct_reposters: 1,
        distinct_positive_engagers: 1,
        engagement_tier: 1,
        engagement_rate: 1.0,
        score: 1.0,
        rank,
    }
}

#[test]
fn creator_is_active_when_latest_published_video_is_within_30_days() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let latest_video = CreatorLatestVideo {
        published_at: Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap(),
    };

    assert!(is_active_creator(now, &latest_video));
}

#[test]
fn creator_is_inactive_when_latest_published_video_is_older_than_30_days() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let latest_video = CreatorLatestVideo {
        published_at: Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap(),
    };

    assert!(!is_active_creator(now, &latest_video));
}

#[test]
fn winner_selection_skips_archive_accounts_until_it_finds_an_active_creator() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let ranked = [
        candidate("archivepubkey", "KingBach", 1),
        candidate("activepubkey", "rabble", 2),
    ];

    let winner = select_first_active_creator(now, ranked.iter(), |pubkey| match pubkey {
        "archivepubkey" => Some(CreatorLatestVideo {
            published_at: Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap(),
        }),
        "activepubkey" => Some(CreatorLatestVideo {
            published_at: Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap(),
        }),
        _ => None,
    })
    .unwrap();

    assert_eq!(winner.pubkey, "activepubkey");
}
