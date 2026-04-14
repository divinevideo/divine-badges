#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicAppAsset {
    BootJs,
    AuthSessionJs,
    NostrConstantsJs,
    NostrRelayJs,
    ViewsCommonJs,
    ViewsMeJs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRouteMatch {
    LandingPage,
    Health,
    Avatar,
    MePage,
    IssuerPubkey,
    AppAsset(PublicAppAsset),
    NotFound,
}

pub fn classify_public_route(path: &str) -> PublicRouteMatch {
    match path {
        "/" => PublicRouteMatch::LandingPage,
        "/healthz" => PublicRouteMatch::Health,
        "/avatar.png" => PublicRouteMatch::Avatar,
        "/me" => PublicRouteMatch::MePage,
        "/pubkey" => PublicRouteMatch::IssuerPubkey,
        "/app/boot.js" => PublicRouteMatch::AppAsset(PublicAppAsset::BootJs),
        "/app/auth/session.js" => PublicRouteMatch::AppAsset(PublicAppAsset::AuthSessionJs),
        "/app/nostr/constants.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrConstantsJs),
        "/app/nostr/relay.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrRelayJs),
        "/app/views/common.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsCommonJs),
        "/app/views/me.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsMeJs),
        _ => PublicRouteMatch::NotFound,
    }
}
