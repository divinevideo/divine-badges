use chrono::{TimeZone, Utc};
use divine_badges::eligibility::{is_active_creator, select_first_active_creator};
use divine_badges::models::{CreatorLatestVideo, LeaderboardCreator};

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
    let ranked = vec![
        LeaderboardCreator {
            pubkey: "archivepubkey".into(),
            display_name: "KingBach".into(),
            name: "KingBach".into(),
            nip05: None,
            picture: "".into(),
            loops: 1100.0,
            views: 1000,
            unique_viewers: 500,
            videos_with_views: 93,
        },
        LeaderboardCreator {
            pubkey: "activepubkey".into(),
            display_name: "rabble".into(),
            name: "rabble".into(),
            nip05: None,
            picture: "".into(),
            loops: 431.0,
            views: 400,
            unique_viewers: 200,
            videos_with_views: 56,
        },
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
