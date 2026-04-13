use divine_badges::awards::award_catalog;
use divine_badges::divine_api::{build_leaderboard_url, parse_leaderboard_response};
use divine_badges::models::LeaderboardCreator;

#[test]
fn award_catalog_contains_three_fixed_creator_awards() {
    let awards = award_catalog();
    assert_eq!(awards.len(), 3);
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_day"));
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_week"));
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_month"));
}

#[test]
fn display_name_falls_back_to_name_then_short_pubkey() {
    let creator = LeaderboardCreator {
        pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
        display_name: "".into(),
        name: "ori3".into(),
        picture: "".into(),
        loops: 136.0,
        views: 100,
        unique_viewers: 50,
        videos_with_views: 21,
    };

    assert_eq!(creator.best_display_name(), "ori3");
}

#[test]
fn leaderboard_url_targets_ranked_candidate_window() {
    let url = build_leaderboard_url("https://api.divine.video", "day", 10).unwrap();
    assert_eq!(
        url.as_str(),
        "https://api.divine.video/api/leaderboard/creators?period=day&limit=10"
    );
}

#[test]
fn parse_leaderboard_response_returns_ranked_entries() {
    let body = r#"{
      "period": "day",
      "entries": [
        {
          "pubkey": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
          "display_name": "Ori3",
          "name": "ori3",
          "picture": "",
          "loops": 136.0,
          "views": 100,
          "unique_viewers": 50,
          "videos_with_views": 21
        }
      ]
    }"#;

    let response = parse_leaderboard_response(body).unwrap();
    assert_eq!(response.entries.len(), 1);
    assert_eq!(response.entries[0].best_display_name(), "Ori3");
    assert_eq!(response.entries[0].loops, 136.0);
}
