use divine_badges::engagement::{broadcast_campaign, winner_campaign};

mod support;
use support::completed_run;

#[test]
fn winner_campaign_addresses_the_winner_by_name() {
    let run = completed_run(Some("KingBach"), Some(&"a".repeat(64)));
    let campaign = winner_campaign(&run).expect("campaign");

    assert_eq!(campaign.segment_type, "explicit_pubkey_list");
    assert_eq!(campaign.recipients, vec!["a".repeat(64)]);
    assert!(campaign.body.contains("Diviner"));
    assert!(campaign.automation_key.contains(&run.period_key));
}

#[test]
fn broadcast_campaign_names_the_winner_and_targets_the_opt_in_audience() {
    let run = completed_run(Some("KingBach"), Some(&"a".repeat(64)));
    let campaign = broadcast_campaign(&run).expect("campaign");

    assert_eq!(campaign.segment_type, "opted_in_push_audience");
    assert!(campaign.recipients.is_empty());
    assert!(campaign.body.contains("KingBach"));
    assert!(campaign.tap_target_value.contains(&"a".repeat(64)));
}

#[test]
fn diviner_payloads_carry_no_personalized_recipients_field() {
    // The digest added personalizedRecipients to the shared campaign body.
    // An empty list must stay absent from the wire, not arrive as [].
    let run = completed_run(Some("KingBach"), Some(&"a".repeat(64)));

    for campaign in [
        winner_campaign(&run).expect("winner campaign"),
        broadcast_campaign(&run).expect("broadcast campaign"),
    ] {
        let payload = serde_json::to_value(&campaign).expect("payload");
        assert!(payload.get("personalizedRecipients").is_none(), "{payload}");
    }
}

#[test]
fn a_run_without_a_winner_produces_no_campaigns() {
    // Review Focus 5.
    let run = completed_run(Some("KingBach"), None);
    assert!(winner_campaign(&run).is_none());
    assert!(broadcast_campaign(&run).is_none());
}

#[test]
fn a_winner_without_a_display_name_gets_neutral_copy() {
    // Review Focus 5: never render an empty name or a raw pubkey to users.
    let run = completed_run(None, Some(&"a".repeat(64)));
    let campaign = broadcast_campaign(&run).expect("campaign");

    assert!(!campaign.body.contains(&"a".repeat(64)));
    assert!(!campaign.body.contains("  "));
    assert!(campaign.body.contains("Today's Diviner"));
}
