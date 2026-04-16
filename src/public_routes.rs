#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicAppAsset {
    BootJs,
    AuthProfileJs,
    AuthSessionJs,
    MediaBlossomJs,
    NostrBadgesJs,
    NostrConstantsJs,
    NostrIdentityJs,
    NostrRelayJs,
    ViewsCommonJs,
    ViewsBadgeJs,
    ViewsMeJs,
    ViewsNewJs,
    ViewsProfileJs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRouteMatch {
    LandingPage,
    Health,
    Avatar,
    MePage,
    NewPage,
    ProfilePage,
    BadgePage,
    IssuerPubkey,
    AppAsset(PublicAppAsset),
    NotFound,
}

pub fn classify_public_route(path: &str) -> PublicRouteMatch {
    if path.starts_with("/p/") && path.len() > 3 {
        return PublicRouteMatch::ProfilePage;
    }
    if path.starts_with("/b/") && path.len() > 3 {
        return PublicRouteMatch::BadgePage;
    }
    match path {
        "/" => PublicRouteMatch::LandingPage,
        "/healthz" => PublicRouteMatch::Health,
        "/avatar.png" => PublicRouteMatch::Avatar,
        "/me" => PublicRouteMatch::MePage,
        "/new" => PublicRouteMatch::NewPage,
        "/pubkey" => PublicRouteMatch::IssuerPubkey,
        "/app/boot.js" => PublicRouteMatch::AppAsset(PublicAppAsset::BootJs),
        "/app/auth/profile.js" => PublicRouteMatch::AppAsset(PublicAppAsset::AuthProfileJs),
        "/app/auth/session.js" => PublicRouteMatch::AppAsset(PublicAppAsset::AuthSessionJs),
        "/app/media/blossom.js" => PublicRouteMatch::AppAsset(PublicAppAsset::MediaBlossomJs),
        "/app/nostr/badges.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrBadgesJs),
        "/app/nostr/constants.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrConstantsJs),
        "/app/nostr/identity.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrIdentityJs),
        "/app/nostr/relay.js" => PublicRouteMatch::AppAsset(PublicAppAsset::NostrRelayJs),
        "/app/views/badge.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsBadgeJs),
        "/app/views/common.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsCommonJs),
        "/app/views/me.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsMeJs),
        "/app/views/new.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsNewJs),
        "/app/views/profile.js" => PublicRouteMatch::AppAsset(PublicAppAsset::ViewsProfileJs),
        _ => PublicRouteMatch::NotFound,
    }
}
