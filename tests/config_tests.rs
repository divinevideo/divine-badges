use divine_badges::config::{creator_link_for_base, validate_base_url};
use url::Url;

const PUBKEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NPUB_LINK: &str =
    "https://divine.video/npub1qy352euf40x77qfrg4ncn27dauqjx3t83x4ummcpydzk0zdtehhstefp92";

#[test]
fn config_requires_non_empty_api_base_url() {
    let result = validate_base_url("");
    assert!(result.is_err());
}

#[test]
fn divine_nip05_uses_divine_subdomain_link() {
    let url = creator_link_for_base("https://divine.video", Some("rabble@divine.video"), PUBKEY);

    assert_eq!(url, "https://rabble.divine.video");
}

#[test]
fn missing_nip05_falls_back_to_npub_link() {
    let url = creator_link_for_base("https://divine.video", None, PUBKEY);

    assert_eq!(url, NPUB_LINK);
}

#[test]
fn unsafe_divine_nip05_local_parts_fall_back_to_the_full_npub_link() {
    let hostile_or_invalid = [
        "evil.example#@divine.video",
        "evil.example/path@divine.video",
        "-leading@divine.video",
        "trailing-@divine.video",
        "has space@divine.video",
        "evil@divine.video@attacker.example",
        "åda@divine.video",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@divine.video",
    ];

    for nip05 in hostile_or_invalid {
        let link = creator_link_for_base("https://divine.video", Some(nip05), PUBKEY);
        assert_eq!(link, NPUB_LINK, "unsafe NIP-05 was accepted: {nip05}");
    }
}

#[test]
fn accepted_divine_nip05_links_have_the_exact_validated_divine_host() {
    let link = creator_link_for_base("https://divine.video", Some("Ada-2@DIVINE.VIDEO"), PUBKEY);
    let parsed = Url::parse(&link).unwrap();

    assert_eq!(link, "https://ada-2.divine.video");
    assert_eq!(parsed.host_str(), Some("ada-2.divine.video"));
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.path(), "/");
    assert_eq!(parsed.query(), None);
    assert_eq!(parsed.fragment(), None);
}
