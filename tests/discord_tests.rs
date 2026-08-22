use divine_badges::discord::{build_announcement_message, build_webhook_payload};

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
fn announcement_message_removes_injected_lines_and_urls_without_breaking_unicode_names() {
    let text = build_announcement_message(
        "Diviner of the Day",
        "  Åda\nhttps://evil.example/profile\t東京\u{0007}  ",
        42,
        9,
        7,
        318,
        "https://divine.video/ada",
    );

    assert_eq!(
        text,
        "Diviner of the Day: Åda 東京 — 42 positive reactors, 9 commenters, 7 reposts, and 318 unique viewers.\nhttps://divine.video/ada"
    );
    assert_eq!(text.lines().count(), 2);
    assert_eq!(text.matches("https://").count(), 1);
    assert!(!text.contains("evil.example"));
}

#[test]
fn announcement_message_uses_a_safe_fallback_when_the_name_sanitizes_empty() {
    let text = build_announcement_message(
        "Diviner of the Day",
        "\nhttps://evil.example\u{0000}",
        1,
        1,
        1,
        1,
        "https://divine.video/ada",
    );

    assert!(text.starts_with("Diviner of the Day: Divine creator —"));
    assert!(!text.contains("evil.example"));
}

#[test]
fn webhook_payload_disables_all_discord_mentions() {
    let payload: serde_json::Value =
        serde_json::from_str(&build_webhook_payload("hello @everyone <@123>")).unwrap();

    assert_eq!(payload["content"], "hello @everyone <@123>");
    assert_eq!(payload["allowed_mentions"]["parse"], serde_json::json!([]));
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
