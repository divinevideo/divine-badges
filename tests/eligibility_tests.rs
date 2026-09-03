use chrono::{Duration, TimeZone, Utc};
use divine_badges::eligibility::{
    is_candidate_active_for_period, is_diviner_award_excluded_creator,
    DIVINER_AWARD_EXCLUDED_PUBKEYS,
};
use divine_badges::nip19::encode_npub;
use std::collections::BTreeSet;

#[test]
fn canonical_candidate_activity_window_is_lower_inclusive_and_end_exclusive() {
    let period_end = Utc.with_ymd_and_hms(2026, 8, 22, 0, 0, 0).unwrap();
    let lower_bound = period_end - Duration::days(30);

    assert!(is_candidate_active_for_period(lower_bound, period_end));
    assert!(!is_candidate_active_for_period(
        lower_bound - Duration::seconds(1),
        period_end
    ));
    assert!(!is_candidate_active_for_period(period_end, period_end));
    assert!(!is_candidate_active_for_period(
        period_end + Duration::seconds(1),
        period_end
    ));
}

#[test]
fn founder_exclusion_is_case_insensitive_and_other_pubkeys_remain_eligible() {
    let founder = "D95AA8FC0EFF8E488952495B8064991D27FB96ED8652F12CDEDC5A4E8B5AE540";
    let creator = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    assert!(is_diviner_award_excluded_creator(founder));
    assert!(!is_diviner_award_excluded_creator(creator));
}

#[test]
fn documented_personnel_accounts_match_the_exclusion_list() {
    let documented_accounts: Vec<(&str, &str)> =
        include_str!("../docs/divine-staff-nostr-accounts.md")
            .lines()
            .filter(|line| line.starts_with("| npub1"))
            .map(|line| {
                let mut columns = line
                    .split('|')
                    .map(str::trim)
                    .filter(|cell| !cell.is_empty());
                let npub = columns.next().expect("documented npub");
                let hex = columns.next().expect("documented hex pubkey");
                assert!(
                    columns.next().is_none(),
                    "unexpected roster column in {line}"
                );
                (npub, hex)
            })
            .collect();

    assert_eq!(
        documented_accounts.len(),
        DIVINER_AWARD_EXCLUDED_PUBKEYS.len()
    );

    let documented_hex: BTreeSet<&str> = documented_accounts
        .iter()
        .map(|(npub, hex)| {
            let encoded = encode_npub(hex).expect("documented hex pubkey should encode");
            assert_eq!(encoded, *npub);
            assert!(is_diviner_award_excluded_creator(hex));
            assert!(is_diviner_award_excluded_creator(&hex.to_uppercase()));
            *hex
        })
        .collect();
    let excluded_hex: BTreeSet<&str> = DIVINER_AWARD_EXCLUDED_PUBKEYS.iter().copied().collect();

    assert_eq!(documented_hex, excluded_hex);
}
