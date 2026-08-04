mod commands;
mod config;
mod handler;
mod health;
mod state;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use config::Config;
use qalqon_core::ModerationStore;
use qalqon_storage::PgModerationStore;
use state::AppState;
use teloxide::{
    Bot,
    dispatching::UpdateFilterExt,
    dptree,
    prelude::{Dispatcher, Requester},
    types::Update,
    utils::command::BotCommands,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return health::probe(Config::health_addr_from_env()?).await;
    }
    init_tracing();

    let config = Config::from_env()?;
    let store: Arc<dyn ModerationStore> = Arc::new(connect_store(&config).await?);
    let mut bot = Bot::new(config.token.clone());
    if let Some(api_url) = config.telegram_api_url.clone() {
        bot = bot.set_api_url(api_url);
    }

    if std::env::args().any(|arg| arg == "--doctor") {
        store.healthcheck().await?;
        let me = bot
            .get_me()
            .await
            .context("Telegram token yoki tarmoq tekshiruvi o'tmadi")?;
        tracing::info!(
            username = me.username(),
            "doctor: database va Telegram tayyor"
        );
        return Ok(());
    }

    let state = AppState::new(
        Arc::clone(&store),
        config.owner_ids,
        Duration::from_secs(config.admin_cache_ttl_secs),
        Duration::from_secs(config.policy_cache_ttl_secs),
    );
    let me = bot.get_me().await?;
    bot.set_my_commands(commands::Command::bot_commands())
        .await
        .context("Telegram command menu ro'yxatdan o'tmadi")?;
    tracing::info!(username = me.username(), "CheklaBot ishga tushdi");

    let schema = Update::filter_message().endpoint(handler::message);
    let health_listener = health::bind(config.health_addr).await?;
    let health_task = tokio::spawn(health::serve(health_listener, store));
    Dispatcher::builder(bot, schema)
        .dependencies(dptree::deps![state, me])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    health_task.abort();
    match health_task.await {
        Err(error) if error.is_cancelled() => {}
        Err(error) => tracing::warn!(%error, "health server task join xatosi"),
        Ok(Err(error)) => tracing::warn!(%error, "health server xato bilan tugadi"),
        Ok(Ok(())) => {}
    }
    tracing::info!("CheklaBot to'xtadi");
    Ok(())
}

async fn connect_store(config: &Config) -> Result<PgModerationStore> {
    let attempts = config.database_connect_max_attempts.max(1);
    for attempt in 1..=attempts {
        match PgModerationStore::connect(&config.database_url, config.database_max_connections)
            .await
        {
            Ok(store) => return Ok(store),
            Err(error) if attempt < attempts => {
                let delay = config
                    .database_connect_backoff_ms
                    .saturating_mul(u64::from(attempt))
                    .min(5_000);
                tracing::warn!(
                    attempt,
                    attempts,
                    delay_ms = delay,
                    %error,
                    "PostgreSQL hali tayyor emas"
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            Err(error) => return Err(error).context("PostgreSQL bilan ulanish amalga oshmadi"),
        }
    }
    unreachable!("attempts is always at least one")
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("qalqon=info,teloxide=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();
}
