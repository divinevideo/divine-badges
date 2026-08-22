use divine_badges::landing_page::{
    render_page, route_path, AwardHistoryEntry, AwardHistorySection, LandingPageView, PublicRoute,
};
use divine_badges::profile::DIVINE_BADGES_PROFILE;
use divine_badges::public_routes::{classify_public_route, PublicAppAsset, PublicRouteMatch};

#[test]
fn render_page_groups_recent_history_by_award() {
    let html = render_page(&LandingPageView {
        sections: vec![
            AwardHistorySection {
                title: "Diviner of the Day",
                description: "Daily winners from the closed UTC day.",
                entries: vec![AwardHistoryEntry {
                    period_key: "2026-04-12".into(),
                    winner_name: "rabble".into(),
                    winner_picture: Some("https://cdn.divine.video/rabble.png".into()),
                    profile_url: "https://rabble.divine.video".into(),
                }],
            },
            AwardHistorySection {
                title: "Diviner of the Week",
                description: "Weekly winners from the closed UTC week.",
                entries: vec![],
            },
            AwardHistorySection {
                title: "Diviner of the Month",
                description: "Monthly winners from the closed UTC month.",
                entries: vec![],
            },
        ],
    });

    assert!(html.contains("Diviner of the Day"));
    assert!(html.contains("Diviner of the Week"));
    assert!(html.contains("Diviner of the Month"));
    assert!(html.contains("rabble"));
    assert!(html.contains("Nothing here yet. Go make some noise."));
    assert!(html.contains("https://rabble.divine.video"));
}

#[test]
fn public_copy_explains_engagement_first_awards_and_exact_utc_periods() {
    let html = render_page(&LandingPageView { sections: vec![] });
    let profile_copy = DIVINE_BADGES_PROFILE.about.to_lowercase();
    let public_copy = format!("{html}\n{profile_copy}").to_lowercase();

    for signal in ["positive reactions", "comments", "reposts"] {
        assert!(
            public_copy.contains(signal),
            "public copy should name {signal}"
        );
        assert!(
            profile_copy.contains(signal),
            "issuer profile should name {signal}"
        );
    }
    assert!(public_copy.contains("positive engagement comes first"));
    assert!(public_copy.contains("reach and loops count for less"));
    assert!(public_copy.contains("repeating the same action does not add more weight"));
    assert!(public_copy.contains("negative reactions do not"));
    assert!(public_copy.contains("if nobody gets positive engagement, reach picks the winner"));

    assert!(public_copy.contains("previous utc day"));
    assert!(public_copy.contains("monday-through-sunday week"));
    assert!(public_copy.contains("calendar month"));

    for loop_only_claim in ["just loops", "most loops", "won with"] {
        assert!(
            !public_copy.contains(loop_only_claim),
            "public copy still contains loop-only claim: {loop_only_claim}"
        );
    }
    assert!(
        !html.contains("<span class=\"unit\">loops</span>"),
        "award cards must not imply that loop count selected the winner"
    );
}

#[test]
fn cron_retries_incomplete_closed_periods_hourly_after_the_first_attempt() {
    let wrangler = include_str!("../wrangler.toml");
    let readme = include_str!("../README.md");

    assert!(wrangler.contains("crons = [\"35 * * * *\"]"));
    assert!(readme.contains("00:35Z is the first attempt"));
    assert!(readme.contains("later hourly invocations are idempotent retries"));
    assert!(readme.contains("incomplete closed-period runs"));
    assert!(readme.contains("publishes the Diviner-of-the-day/week/month awards to the relay configured by `DIVINE_RELAY_URL`"));
}

#[test]
fn mobile_winner_cards_do_not_reserve_a_removed_score_row() {
    let html = render_page(&LandingPageView { sections: vec![] });

    assert!(!html.contains("grid-template-rows:auto auto"));
}

#[test]
fn route_path_maps_root_health_and_unknown_paths() {
    assert_eq!(route_path("/"), PublicRoute::LandingPage);
    assert_eq!(route_path("/healthz"), PublicRoute::Health);
    assert_eq!(route_path("/missing"), PublicRoute::NotFound);
}

#[test]
fn classify_public_route_matches_shared_app_assets() {
    assert_eq!(
        classify_public_route("/app/boot.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::BootJs)
    );
    assert_eq!(
        classify_public_route("/app/auth/profile.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::AuthProfileJs)
    );
    assert_eq!(
        classify_public_route("/app/auth/return_to.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::AuthReturnToJs)
    );
    assert_eq!(
        classify_public_route("/app/media/blossom.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::MediaBlossomJs)
    );
    assert_eq!(
        classify_public_route("/app/views/me_empty_state.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::ViewsMeEmptyStateJs)
    );
    assert_eq!(
        classify_public_route("/app/views/new_text_fields.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::ViewsNewTextFieldsJs)
    );
    assert_eq!(
        classify_public_route("/app/nostr/publish.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::NostrPublishJs)
    );
    assert_eq!(
        classify_public_route("/app/nostr/profile_metadata.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::NostrProfileMetadataJs)
    );
    assert_eq!(classify_public_route("/new"), PublicRouteMatch::NewPage);
    assert_eq!(
        classify_public_route("/auth/callback"),
        PublicRouteMatch::AuthCallbackPage
    );
    assert_eq!(
        classify_public_route("/p/npub1example"),
        PublicRouteMatch::ProfilePage
    );
    assert_eq!(
        classify_public_route("/b/naddr1example"),
        PublicRouteMatch::BadgePage
    );
}

#[test]
fn classify_public_route_matches_badge_edit_page() {
    assert_eq!(
        classify_public_route("/b/naddr1example/edit"),
        PublicRouteMatch::BadgeEditPage
    );
    assert_eq!(
        classify_public_route("/b/naddr1example"),
        PublicRouteMatch::BadgePage
    );
    assert_eq!(
        classify_public_route("/app/views/edit_badge.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::ViewsEditBadgeJs)
    );
    assert_eq!(
        classify_public_route("/app/views/markdown.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::ViewsMarkdownJs)
    );
}

#[test]
fn classify_public_route_matches_relays_page_and_asset() {
    assert_eq!(
        classify_public_route("/relays"),
        PublicRouteMatch::RelaysPage
    );
    assert_eq!(
        classify_public_route("/app/views/relays.js"),
        PublicRouteMatch::AppAsset(PublicAppAsset::ViewsRelaysJs)
    );
}
