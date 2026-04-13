use divine_badges::config::{creator_link_for_base, validate_base_url};

#[test]
fn config_requires_non_empty_api_base_url() {
    let result = validate_base_url("");
    assert!(result.is_err());
}

#[test]
fn divine_nip05_uses_divine_subdomain_link() {
    let url = creator_link_for_base(
        "https://divine.video",
        Some("rabble@divine.video"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    assert_eq!(url, "https://rabble.divine.video");
}

#[test]
fn missing_nip05_falls_back_to_npub_link() {
    let url = creator_link_for_base(
        "https://divine.video",
        None,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    assert_eq!(
        url,
        "https://divine.video/npub1qy352euf40x77qfrg4ncn27dauqjx3t83x4ummcpydzk0zdtehhstefp92"
    );
}
