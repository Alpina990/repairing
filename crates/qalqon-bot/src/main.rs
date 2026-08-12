mod api;
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
    payloads::SetChatMenuButtonSetters,
    prelude::{Dispatcher, Requester},
    types::{MenuButton, Update, WebAppInfo},
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
    let pg_store = connect_store(&config).await?;
    let store: Arc<dyn ModerationStore> = Arc::new(pg_store.clone());
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
        validate_bot_identity(me.username(), &config.expected_bot_username)?;
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
    validate_bot_identity(me.username(), &config.expected_bot_username)?;
    bot.set_my_commands(commands::Command::bot_commands())
        .await
        .context("Telegram command menu ro'yxatdan o'tmadi")?;
    let mini_app = config.mini_app_url.clone().map(|url| WebAppInfo { url });
    if let Some(web_app) = mini_app.clone() {
        let url = web_app.url.clone();
        bot.set_chat_menu_button()
            .menu_button(MenuButton::WebApp {
                text: "Boshqaruv".into(),
                web_app,
            })
            .await
            .context("Telegram Mini App menu tugmasi ro'yxatdan o'tmadi")?;
        tracing::info!(%url, "Telegram Mini App menu tugmasi sozlandi");
    }
    tracing::info!(username = me.username(), "CheklaBot ishga tushdi");

    let schema = dptree::entry()
        .branch(Update::filter_message().endpoint(handler::message))
        .branch(Update::filter_edited_message().endpoint(handler::edited_message))
        .branch(Update::filter_chat_member().endpoint(handler::chat_member));
    let health_listener = health::bind(config.health_addr).await?;
    let api_listener = api::bind(config.api_addr).await?;
    let health_task = tokio::spawn(health::serve(health_listener, store));
    let api_state = api::ApiState::new(
        bot.clone(),
        me.clone(),
        state.clone(),
        pg_store.clone(),
        config.token,
        Duration::from_secs(config.mini_app_auth_max_age_secs),
    );
    let api_origin = config.mini_app_origin.clone();
    let api_task =
        tokio::spawn(
            async move { api::serve(api_listener, api_state, api_origin.as_deref()).await },
        );
    Dispatcher::builder(bot, schema)
        .dependencies(dptree::deps![state, me, pg_store.clone(), mini_app])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    health_task.abort();
    api_task.abort();
    match health_task.await {
        Err(error) if error.is_cancelled() => {}
        Err(error) => tracing::warn!(%error, "health server task join xatosi"),
        Ok(Err(error)) => tracing::warn!(%error, "health server xato bilan tugadi"),
        Ok(Ok(())) => {}
    }
    match api_task.await {
        Err(error) if error.is_cancelled() => {}
        Err(error) => tracing::warn!(%error, "Mini App API task join xatosi"),
        Ok(Err(error)) => tracing::warn!(%error, "Mini App API xato bilan tugadi"),
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

fn validate_bot_identity(actual: &str, expected: &str) -> Result<()> {
    let actual = actual.trim().trim_start_matches('@');
    let expected = expected.trim().trim_start_matches('@');
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    anyhow::bail!(
        "Telegram token @{actual} botiga tegishli; bu loyiha uchun @{expected} tokeni kutilgan"
    )
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn accepts_only_the_configured_telegram_bot_username() {
        assert!(validate_bot_identity("chekla_qalqon_bot", "chekla_qalqon_bot").is_ok());
        assert!(validate_bot_identity("@Chekla_Qalqon_Bot", "chekla_qalqon_bot").is_ok());
        assert!(validate_bot_identity("CheklaGuardbot", "chekla_qalqon_bot").is_err());
    }
}
