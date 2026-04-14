#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use worker::{Context, Env, Method, Request, Response, ScheduleContext, ScheduledEvent};

    use crate::clock::{Clock, SystemClock};
    use crate::config::{binding_string, AppConfig};
    use crate::discord::WasmDiscordClient;
    use crate::divine_api::{WasmActivityClient, WasmLeaderboardClient};
    use crate::landing_page::{build_view, render_page};
    use crate::nip19::encode_npub;
    use crate::profile::{build_profile_event, DIVINE_BADGES_PROFILE};
    use crate::public_routes::{classify_public_route, PublicAppAsset, PublicRouteMatch};
    use crate::relay_client::WasmRelayClient;
    use crate::repository::D1AwardRepository;
    use crate::use_cases::run_award_tick;

    #[worker::event(fetch, respond_with_errors)]
    pub async fn fetch(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
        let method = req.method();
        let path = req.path();
        match (method, classify_public_route(path.as_str())) {
            (Method::Get, PublicRouteMatch::LandingPage) => serve_landing_page(env).await,
            (Method::Get, PublicRouteMatch::Health) => Response::ok("ok"),
            (Method::Get, PublicRouteMatch::Avatar) => serve_avatar(),
            (Method::Get, PublicRouteMatch::MePage) => serve_me(),
            (Method::Get, PublicRouteMatch::IssuerPubkey) => serve_pubkey(env).await,
            (Method::Get, PublicRouteMatch::AppAsset(asset)) => serve_app_asset(asset),
            (Method::Post, _) if path == "/admin/publish-profile" => {
                publish_profile(req, env).await
            }
            _ => Response::error("Not Found", 404),
        }
    }

    #[worker::event(scheduled)]
    pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
        if let Err(error) = run_scheduled(env).await {
            worker::console_error!("{}", error);
        }
    }

    async fn run_scheduled(env: Env) -> Result<(), String> {
        let config = AppConfig::from_env(&env).map_err(|error| error.to_string())?;
        let database = env.d1("DB").map_err(|error| error.to_string())?;
        let repository = D1AwardRepository::new(database);
        let leaderboard = WasmLeaderboardClient::new(config.divine_api_base_url.clone());
        let activity = WasmActivityClient::new(config.divine_api_base_url.clone());
        let publisher =
            WasmRelayClient::new(config.divine_relay_url.clone(), &config.nostr_issuer_nsec)
                .map_err(|error| error.to_string())?;
        let discord = WasmDiscordClient::new(config.discord_webhook_url.clone());
        let clock = SystemClock;

        run_award_tick(
            clock.now(),
            &config,
            &repository,
            &leaderboard,
            &activity,
            &publisher,
            &discord,
        )
        .await
        .map_err(|error| error.to_string())?;

        Ok(())
    }

    async fn serve_landing_page(env: Env) -> worker::Result<Response> {
        let database = env.d1("DB")?;
        let repository = D1AwardRepository::new(database);
        let creator_base_url = binding_string(&env, "DIVINE_CREATOR_BASE_URL")
            .unwrap_or_else(|_| "https://divine.video".into());
        match build_view(&repository, &creator_base_url).await {
            Ok(view) => Response::from_html(render_page(&view)),
            Err(_) => Response::error("Internal Server Error", 500),
        }
    }

    const AVATAR_PNG: &[u8] = include_bytes!("../assets/avatar.png");
    const ME_PAGE: &str = include_str!("../assets/me.html");
    const APP_BOOT_JS: &str = include_str!("../assets/app/boot.js");
    const APP_AUTH_SESSION_JS: &str = include_str!("../assets/app/auth/session.js");
    const APP_NOSTR_CONSTANTS_JS: &str = include_str!("../assets/app/nostr/constants.js");
    const APP_NOSTR_RELAY_JS: &str = include_str!("../assets/app/nostr/relay.js");
    const APP_VIEWS_COMMON_JS: &str = include_str!("../assets/app/views/common.js");
    const APP_VIEWS_ME_JS: &str = include_str!("../assets/app/views/me.js");

    fn serve_avatar() -> worker::Result<Response> {
        let mut response = Response::from_bytes(AVATAR_PNG.to_vec())?;
        response.headers_mut().set("content-type", "image/png")?;
        response
            .headers_mut()
            .set("cache-control", "public, max-age=86400, immutable")?;
        Ok(response)
    }

    fn serve_me() -> worker::Result<Response> {
        Response::from_html(ME_PAGE)
    }

    fn serve_app_asset(asset: PublicAppAsset) -> worker::Result<Response> {
        let source = match asset {
            PublicAppAsset::BootJs => APP_BOOT_JS,
            PublicAppAsset::AuthSessionJs => APP_AUTH_SESSION_JS,
            PublicAppAsset::NostrConstantsJs => APP_NOSTR_CONSTANTS_JS,
            PublicAppAsset::NostrRelayJs => APP_NOSTR_RELAY_JS,
            PublicAppAsset::ViewsCommonJs => APP_VIEWS_COMMON_JS,
            PublicAppAsset::ViewsMeJs => APP_VIEWS_ME_JS,
        };
        let mut response = Response::ok(source)?;
        response
            .headers_mut()
            .set("content-type", "application/javascript; charset=utf-8")?;
        response
            .headers_mut()
            .set("cache-control", "public, max-age=300")?;
        Ok(response)
    }

    async fn serve_pubkey(env: Env) -> worker::Result<Response> {
        let nsec = match binding_string(&env, "NOSTR_ISSUER_NSEC") {
            Ok(value) => value,
            Err(_) => return Response::error("issuer nsec not configured", 500),
        };
        let relay_url = binding_string(&env, "DIVINE_RELAY_URL")
            .unwrap_or_else(|_| "wss://relay.divine.video".into());
        let client = match WasmRelayClient::new(relay_url, &nsec) {
            Ok(client) => client,
            Err(error) => return Response::error(error.to_string(), 500),
        };
        let hex = client.public_key();
        let npub = encode_npub(&hex).unwrap_or_else(|_| hex.clone());
        let body = serde_json::json!({ "hex": hex, "npub": npub }).to_string();
        let mut response = Response::ok(body)?;
        response
            .headers_mut()
            .set("content-type", "application/json")?;
        Ok(response)
    }

    async fn publish_profile(req: Request, env: Env) -> worker::Result<Response> {
        let expected = match binding_string(&env, "ADMIN_TOKEN") {
            Ok(value) => value,
            Err(_) => return Response::error("admin token not configured", 500),
        };
        let presented = req
            .headers()
            .get("authorization")
            .ok()
            .flatten()
            .and_then(|value| value.strip_prefix("Bearer ").map(|s| s.to_string()))
            .unwrap_or_default();
        if presented.is_empty() || !constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
            return Response::error("unauthorized", 401);
        }

        let nsec = match binding_string(&env, "NOSTR_ISSUER_NSEC") {
            Ok(value) => value,
            Err(_) => return Response::error("issuer nsec not configured", 500),
        };
        let relay_url = binding_string(&env, "DIVINE_RELAY_URL")
            .unwrap_or_else(|_| "wss://relay.divine.video".into());
        let client = match WasmRelayClient::new(relay_url, &nsec) {
            Ok(client) => client,
            Err(error) => return Response::error(error.to_string(), 500),
        };

        let event = build_profile_event(&DIVINE_BADGES_PROFILE);
        match client.publish_unsigned(event).await {
            Ok(signed) => {
                let body = serde_json::json!({
                    "published": true,
                    "id": signed.id,
                    "pubkey": signed.pubkey,
                    "kind": signed.kind,
                })
                .to_string();
                let mut response = Response::ok(body)?;
                response
                    .headers_mut()
                    .set("content-type", "application/json")?;
                Ok(response)
            }
            Err(error) => Response::error(error.to_string(), 502),
        }
    }

    fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut diff: u8 = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}
