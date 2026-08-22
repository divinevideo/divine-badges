use chrono::{TimeZone, Utc};
use divine_badges::awards::award_catalog;
use divine_badges::divine_api::{
    build_diviner_candidates_url, parse_diviner_candidates_response, ranked_candidates_for_period,
};
use divine_badges::error::AppError;
use divine_badges::models::DivinerCandidate;

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
fn display_name_falls_back_to_name_then_pubkey() {
    let creator = DivinerCandidate {
        pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
        display_name: "".into(),
        name: "ori3".into(),
        nip05: None,
        picture: "".into(),
        loops: 136.0,
        views: 100,
        unique_viewers: 50,
        videos_with_views: 21,
        positive_reactors: 10,
        distinct_commenters: 9,
        distinct_reposters: 8,
        distinct_positive_engagers: 20,
        engagement_tier: 1,
        engagement_rate: 0.4,
        score: 87.5,
        rank: 1,
    };

    assert_eq!(creator.best_display_name(), "ori3");

    let creator = DivinerCandidate {
        name: "".into(),
        ..creator
    };
    assert_eq!(
        creator.best_display_name(),
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    );
}

#[test]
fn divine_api_url_targets_exact_period_candidate_window_with_canonical_utc_bounds() {
    let start = Utc.with_ymd_and_hms(2026, 8, 21, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap();
    let url = build_diviner_candidates_url("https://api.divine.video", start, end, 10).unwrap();

    assert_eq!(
        url.as_str(),
        "https://api.divine.video/api/awards/diviner-candidates?start=2026-08-21T00%3A00%3A00Z&end=2026-08-22T00%3A00%3A00Z&limit=10"
    );
}

#[test]
fn divine_api_parsing_preserves_complete_ranking_receipt() {
    let body = r#"{
      "start": "2026-08-21T00:00:00Z",
      "end": "2026-08-22T00:00:00Z",
      "entries": [
        {
          "pubkey": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
          "display_name": "Ori3",
          "name": "ori3",
          "nip05": "ori3@divine.video",
          "picture": "",
          "loops": 136.0,
          "views": 18446744073709551615,
          "unique_viewers": 9223372036854775808,
          "videos_with_views": 21,
          "positive_reactors": 4294967296,
          "distinct_commenters": 300,
          "distinct_reposters": 200,
          "distinct_positive_engagers": 1750,
          "engagement_tier": 1,
          "engagement_rate": 0.4375,
          "score": 100.0,
          "rank": 1
        }
      ]
    }"#;

    let response = parse_diviner_candidates_response(body).unwrap();
    assert_eq!(
        response.start,
        Utc.with_ymd_and_hms(2026, 8, 21, 0, 0, 0).unwrap()
    );
    assert_eq!(
        response.end,
        Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap()
    );
    assert_eq!(response.entries.len(), 1);
    assert_eq!(response.entries[0].best_display_name(), "Ori3");
    assert_eq!(
        response.entries[0].nip05.as_deref(),
        Some("ori3@divine.video")
    );
    assert_eq!(response.entries[0].loops, 136.0);
    assert_eq!(response.entries[0].views, u64::MAX);
    assert_eq!(response.entries[0].unique_viewers, (i64::MAX as u64) + 1);
    assert_eq!(response.entries[0].positive_reactors, 4_294_967_296);
    assert_eq!(response.entries[0].distinct_commenters, 300);
    assert_eq!(response.entries[0].distinct_reposters, 200);
    assert_eq!(response.entries[0].distinct_positive_engagers, 1_750);
    assert_eq!(response.entries[0].engagement_tier, 1);
    assert_eq!(response.entries[0].engagement_rate, 0.4375);
    assert_eq!(response.entries[0].score, 100.0);
    assert_eq!(response.entries[0].rank, 1);
}

#[test]
fn divine_api_parsing_rejects_partial_score_receipt() {
    let body = r#"{
      "start": "2026-08-21T00:00:00Z",
      "end": "2026-08-22T00:00:00Z",
      "entries": [{
        "pubkey": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "name": "ori3",
        "display_name": "Ori3",
        "nip05": null,
        "picture": "",
        "views": 100,
        "unique_viewers": 50,
        "loops": 136.0,
        "videos_with_views": 21,
        "positive_reactors": 10,
        "distinct_commenters": 9,
        "distinct_reposters": 8,
        "distinct_positive_engagers": 20,
        "engagement_tier": 1,
        "engagement_rate": 0.4,
        "rank": 1
      }]
    }"#;

    let error = parse_diviner_candidates_response(body).unwrap_err();
    assert!(error.to_string().contains("score"));
}

#[test]
fn divine_api_rejects_non_success_responses() {
    let start = Utc.with_ymd_and_hms(2026, 8, 21, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap();

    let error = futures::executor::block_on(ranked_candidates_for_period(
        |_| Ok((503, "service unavailable".into())),
        "https://api.divine.video",
        start,
        end,
        10,
    ))
    .unwrap_err();

    assert!(matches!(error, AppError::Api(_)));
    assert!(error.to_string().contains("503"));
}

#[test]
fn divine_api_rejects_empty_response_with_exact_bounds() {
    let start = Utc.with_ymd_and_hms(2026, 8, 21, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap();
    let body = r#"{
      "start": "2026-08-21T00:00:00Z",
      "end": "2026-08-22T00:00:00Z",
      "entries": []
    }"#;

    let error = futures::executor::block_on(ranked_candidates_for_period(
        |_| Ok((200, body.into())),
        "https://api.divine.video",
        start,
        end,
        10,
    ))
    .unwrap_err();

    assert!(matches!(error, AppError::EmptyLeaderboard(_)));
    assert!(error.to_string().contains("2026-08-21T00:00:00Z"));
    assert!(error.to_string().contains("2026-08-22T00:00:00Z"));
}

#[test]
fn divine_api_rejects_response_for_a_different_exact_period() {
    let start = Utc.with_ymd_and_hms(2026, 8, 21, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap();
    let body = r#"{
      "start": "2026-08-20T00:00:00Z",
      "end": "2026-08-21T00:00:00Z",
      "entries": [{
        "pubkey": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "name": "ori3",
        "display_name": "Ori3",
        "nip05": null,
        "picture": "",
        "views": 100,
        "unique_viewers": 50,
        "loops": 136.0,
        "videos_with_views": 21,
        "positive_reactors": 10,
        "distinct_commenters": 9,
        "distinct_reposters": 8,
        "distinct_positive_engagers": 20,
        "engagement_tier": 1,
        "engagement_rate": 0.4,
        "score": 87.5,
        "rank": 1
      }]
    }"#;

    let error = futures::executor::block_on(ranked_candidates_for_period(
        |_| Ok((200, body.into())),
        "https://api.divine.video",
        start,
        end,
        10,
    ))
    .unwrap_err();

    assert!(matches!(error, AppError::Api(_)));
    assert!(error
        .to_string()
        .contains("expected 2026-08-21T00:00:00Z..2026-08-22T00:00:00Z"));
    assert!(error
        .to_string()
        .contains("received 2026-08-20T00:00:00Z..2026-08-21T00:00:00Z"));
}
