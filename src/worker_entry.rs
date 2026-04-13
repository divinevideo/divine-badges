#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use worker::{Context, Env, Request, Response, ScheduleContext, ScheduledEvent};

    use crate::clock::{Clock, SystemClock};
    use crate::config::{binding_string, AppConfig};
    use crate::discord::WasmDiscordClient;
    use crate::divine_api::{WasmActivityClient, WasmLeaderboardClient};
    use crate::landing_page::{build_view, render_page, route_path, PublicRoute};
    use crate::relay_client::WasmRelayClient;
    use crate::repository::D1AwardRepository;
    use crate::use_cases::run_award_tick;

    #[worker::event(fetch, respond_with_errors)]
    pub async fn fetch(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
        match route_path(req.path().as_str()) {
            PublicRoute::LandingPage => serve_landing_page(env).await,
            PublicRoute::Health => Response::ok("ok"),
            PublicRoute::NotFound => Response::error("Not Found", 404),
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
}
