use divine_badges::nostr::build_badge_award_tags;

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
