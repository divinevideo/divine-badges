use chrono::{DateTime, Duration, Utc};

// Divine personnel accounts; Diviner awards should go to community creators, not the team.
pub const DIVINER_AWARD_EXCLUDED_PUBKEYS: [&str; 10] = [
    "d95aa8fc0eff8e488952495b8064991d27fb96ed8652f12cdedc5a4e8b5ae540",
    "9cfa7d570af5dde1d79b476ac2c8b543b1970065d5488f4d86faf3c34c167e31",
    "fc7031a810ce4b02b6195a7e477cfe3d08c0386038bd45b4431f82d9b3f5ffb0",
    "0edc2f474484769bc9bf6d471d180e4e280b0bcd719b6da791001beb730cff1b",
    "a7e9d5d90b17fad6e4526ce58f0af997b8c14826ecbdbc2c3955e6e258f4000c",
    "ae35ec87e49f3ca2c92a0bbb40c507ae4a6a8e01de29eb9518bc941ed285f943",
    "42f065722892770a07a894cd9c0a3956709785d50c553796e1f70ba6bd3e239f",
    "d8448393e3b69c7ab5511ca3b53b672e587d6f93d6d03553cbf228313f1c30a0",
    "89ef92b9ebe6dc1e4ea398f6477f227e95429627b0a33dc89b640e137b256be5",
    "295dbec79ee785496f703c9648f246665d46839c1d5f582c0342b4583da5ccb4",
];

pub fn is_candidate_active_for_period(
    latest_eligible_publication_at: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> bool {
    let Some(active_since) = period_end.checked_sub_signed(Duration::days(30)) else {
        return false;
    };

    latest_eligible_publication_at >= active_since && latest_eligible_publication_at < period_end
}

pub fn is_diviner_award_excluded_creator(pubkey: &str) -> bool {
    DIVINER_AWARD_EXCLUDED_PUBKEYS
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(pubkey.trim()))
}
