#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use worker::{Context, Env, Request, Response, ScheduleContext, ScheduledEvent};

    use crate::clock::{Clock, SystemClock};
    use crate::config::AppConfig;
    use crate::discord::WasmDiscordClient;
    use crate::divine_api::{WasmActivityClient, WasmLeaderboardClient};
    use crate::relay_client::WasmRelayClient;
    use crate::repository::D1AwardRepository;
    use crate::use_cases::run_award_tick;

    #[worker::event(fetch, respond_with_errors)]
    pub async fn fetch(_req: Request, _env: Env, _ctx: Context) -> worker::Result<Response> {
        Response::ok("divine-badges worker")
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
}
