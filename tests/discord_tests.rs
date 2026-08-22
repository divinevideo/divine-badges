use divine_badges::discord::build_announcement_message;

#[test]
fn announcement_message_explains_the_persisted_engagement_receipt() {
    let text = build_announcement_message(
        "Diviner of the Day",
        "Ada",
        42,
        9,
        7,
        318,
        "https://divine.video/ada",
    );

    assert_eq!(
        text,
        "Diviner of the Day: Ada — 42 positive reactors, 9 commenters, 7 reposts, and 318 unique viewers.\nhttps://divine.video/ada"
    );
    assert!(!text.contains("won with"));
    assert!(!text.contains("loops"));
}

#[test]
fn announcement_message_pluralizes_each_engagement_signal_independently() {
    let fixtures = [
        (
            (1, 2, 3, 4),
            "1 positive reactor, 2 commenters, 3 reposts, and 4 unique viewers",
        ),
        (
            (2, 1, 3, 4),
            "2 positive reactors, 1 commenter, 3 reposts, and 4 unique viewers",
        ),
        (
            (2, 3, 1, 4),
            "2 positive reactors, 3 commenters, 1 repost, and 4 unique viewers",
        ),
        (
            (2, 3, 4, 1),
            "2 positive reactors, 3 commenters, 4 reposts, and 1 unique viewer",
        ),
    ];

    for ((reactors, commenters, reposts, viewers), expected_receipt) in fixtures {
        let text = build_announcement_message(
            "Diviner of the Day",
            "Ada",
            reactors,
            commenters,
            reposts,
            viewers,
            "https://divine.video/ada",
        );

        assert_eq!(
            text,
            format!("Diviner of the Day: Ada — {expected_receipt}.\nhttps://divine.video/ada")
        );
    }
}
