use std::time::{Duration, Instant};

use anyhow::Result;
use qalqon_core::{
    ContentDecision, ContentPolicy, FloodKey, Sanction,
    template::{WelcomeContext, render_welcome},
};
use teloxide::{
    Bot,
    prelude::Requester,
    types::{Me, Message},
    utils::command::BotCommands,
};

use crate::{
    commands::{self, Command},
    state::AppState,
};

pub async fn message(bot: Bot, msg: Message, me: Me, state: AppState) -> Result<()> {
    if let Some(members) = msg.new_chat_members() {
        welcome(&bot, &msg, members, &state).await?;
        return Ok(());
    }

    if let Some(text) = msg.text() {
        if let Ok(command) = Command::parse(text, me.username()) {
            if let Err(error) =
                commands::handle(bot.clone(), msg.clone(), command, state.clone()).await
            {
                tracing::info!(%error, chat_id = msg.chat.id.0, "command rad etildi");
                bot.send_message(msg.chat.id, format!("Xato: {error}"))
                    .await?;
            }
            return Ok(());
        }
    }

    moderate(bot, msg, state).await
}

async fn welcome(
    bot: &Bot,
    msg: &Message,
    members: &[teloxide::types::User],
    state: &AppState,
) -> Result<()> {
    if !msg.chat.is_group() && !msg.chat.is_supergroup() {
        return Ok(());
    }
    let settings = state.store.settings(msg.chat.id.0).await?;
    if !settings.welcome_enabled {
        return Ok(());
    }
    let title = msg.chat.title().unwrap_or("guruh");
    for member in members.iter().filter(|member| !member.is_bot) {
        let text = render_welcome(
            &settings.welcome_template,
            &WelcomeContext {
                first_name: &member.first_name,
                username: member.username.as_deref(),
                user_id: member.id.0,
                chat_title: title,
            },
        );
        bot.send_message(msg.chat.id, text).await?;
    }
    Ok(())
}

async fn moderate(bot: Bot, msg: Message, state: AppState) -> Result<()> {
    if !msg.chat.is_group() && !msg.chat.is_supergroup() {
        return Ok(());
    }
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };
    if user.is_bot || state.is_admin(&bot, msg.chat.id, user.id, false).await? {
        return Ok(());
    }

    let (settings, blocked_terms) = state.policy(msg.chat.id.0).await?;
    let flood = state.flood.observe(
        FloodKey {
            chat_id: msg.chat.id.0,
            user_id: user.id.0,
        },
        settings.flood_limit,
        Duration::from_secs(settings.flood_window_secs),
        Instant::now(),
    );

    if flood {
        bot.delete_message(msg.chat.id, msg.id).await?;
        punish(bot, msg, state, settings.flood_action, "anti-flood").await?;
        return Ok(());
    }

    let Some(text) = msg.text().or_else(|| msg.caption()) else {
        return Ok(());
    };
    if let ContentDecision::Block { matched_term } = ContentPolicy::evaluate(text, &blocked_terms) {
        bot.delete_message(msg.chat.id, msg.id).await?;
        commands::warn_user(
            &bot,
            &state,
            &msg,
            user,
            None,
            &format!("blocklist: {matched_term}"),
        )
        .await?;
    }
    Ok(())
}

async fn punish(
    bot: Bot,
    msg: Message,
    state: AppState,
    sanction: Sanction,
    reason: &str,
) -> Result<()> {
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };
    match sanction {
        Sanction::Delete => {}
        Sanction::Warn => commands::warn_user(&bot, &state, &msg, user, None, reason).await?,
        Sanction::Mute | Sanction::Ban => {
            let settings = state.store.settings(msg.chat.id.0).await?;
            commands::execute_sanction(
                &bot,
                msg.chat.id,
                user.id,
                sanction,
                settings.mute_duration_secs,
            )
            .await?;
            bot.send_message(
                msg.chat.id,
                format!("{}: {} ({reason}).", user.first_name, sanction.as_str()),
            )
            .await?;
            commands::audit_best_effort(
                state.store.as_ref(),
                msg.chat.id.0,
                None,
                user.id.0,
                sanction.as_str(),
                Some(reason),
            )
            .await;
        }
    }
    Ok(())
}
