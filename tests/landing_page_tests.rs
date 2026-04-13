use divine_badges::landing_page::{
    render_page, route_path, AwardHistoryEntry, AwardHistorySection, LandingPageView, PublicRoute,
};

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
                    loops: Some(321.0),
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
    assert!(html.contains("No awards issued yet"));
    assert!(html.contains("https://rabble.divine.video"));
}

#[test]
fn route_path_maps_root_health_and_unknown_paths() {
    assert_eq!(route_path("/"), PublicRoute::LandingPage);
    assert_eq!(route_path("/healthz"), PublicRoute::Health);
    assert_eq!(route_path("/missing"), PublicRoute::NotFound);
}
