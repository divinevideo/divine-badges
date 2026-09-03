use divine_badges::nostr::{build_badge_award_tags, SignedNostrEvent};

#[test]
fn badge_award_tags_include_awardee_and_period_key() {
    let tags = build_badge_award_tags(
        "30009:issuerpubkey:diviner-of-the-day",
        "winnerpubkey",
        "2026-04-12",
    );

    assert!(tags.iter().any(|tag| {
        tag == &vec![
            String::from("a"),
            String::from("30009:issuerpubkey:diviner-of-the-day"),
        ]
    }));
    assert!(tags
        .iter()
        .any(|tag| tag == &vec![String::from("p"), String::from("winnerpubkey")]));
    assert!(tags
        .iter()
        .any(|tag| tag == &vec![String::from("period"), String::from("2026-04-12")]));
}

#[test]
fn signed_nostr_event_round_trips_through_persisted_json() {
    let event = SignedNostrEvent {
        id: "a".repeat(64),
        pubkey: "b".repeat(64),
        created_at: 1_787_356_800,
        kind: 8,
        content: String::new(),
        tags: vec![vec!["p".into(), "c".repeat(64)]],
        sig: "d".repeat(128),
    };

    let json = serde_json::to_string(&event).unwrap();
    let decoded: SignedNostrEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, event);
}
