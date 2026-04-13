use divine_badges::discord::build_announcement_message;

#[test]
fn announcement_message_includes_award_name_loops_and_link() {
    let text = build_announcement_message(
        "Diviner of the Day",
        "Ori3",
        136.0,
        "https://divine.video/u/abcdef",
    );

    assert!(text.contains("Diviner of the Day"));
    assert!(text.contains("Ori3"));
    assert!(text.contains("136"));
    assert!(text.contains("https://divine.video/u/abcdef"));
}
