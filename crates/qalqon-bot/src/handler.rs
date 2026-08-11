use std::time::{Duration, Instant};

use anyhow::Result;
use qalqon_core::{
    ContentDecision, ContentPolicy, FloodKey, Sanction,
    template::{WelcomeContext, render_welcome},
};
use qalqon_storage::{MemberUpsert, PgModerationStore, RichAuditEvent};
use serde_json::json;
use teloxide::{
    Bot,
    prelude::Requester,
    types::{ChatMemberKind, ChatMemberUpdated, Me, Message, Update, WebAppInfo},
    utils::command::BotCommands,
};

use crate::{
    commands::{self, Command},
    state::AppState,
};

pub async fn message(
    bot: Bot,
    msg: Message,
    update: Update,
    me: Me,
    state: AppState,
    store: PgModerationStore,
    mini_app: Option<WebAppInfo>,
) -> Result<()> {
    index_message(&store, &msg).await?;
    if let Some(members) = msg.new_chat_members() {
        for member in members {
            index_user(&store, &msg, member).await?;
        }
        welcome(&bot, &msg, members, &state).await?;
        return Ok(());
    }

    if let Some(text) = msg.text() {
        if let Ok(command) = Command::parse(text, me.username()) {
            if let Err(error) = commands::handle(
                bot.clone(),
                msg.clone(),
                command,
                state.clone(),
                store.clone(),
                mini_app,
            )
            .await
            {
                tracing::info!(%error, chat_id = msg.chat.id.0, "command rad etildi");
                bot.send_message(msg.chat.id, format!("Xato: {error}"))
                    .await?;
            }
            return Ok(());
        }
    }

    moderate(bot, msg, state, store, i64::from(update.id.0), true).await
}

pub async fn edited_message(
    bot: Bot,
    msg: Message,
    update: Update,
    state: AppState,
    store: PgModerationStore,
) -> Result<()> {
    index_message(&store, &msg).await?;
    moderate(bot, msg, state, store, i64::from(update.id.0), false).await
}

async fn index_message(store: &PgModerationStore, msg: &Message) -> Result<()> {
    if (!msg.chat.is_group() && !msg.chat.is_supergroup()) || msg.from.is_none() {
        return Ok(());
    }
    index_user(store, msg, msg.from.as_ref().expect("checked above")).await
}

async fn index_user(
    store: &PgModerationStore,
    msg: &Message,
    user: &teloxide::types::User,
) -> Result<()> {
    let value = MemberUpsert {
        chat_id: msg.chat.id.0,
        chat_title: msg.chat.title().unwrap_or("Telegram guruhi"),
        chat_username: msg.chat.username(),
        chat_type: if msg.chat.is_supergroup() {
            "supergroup"
        } else {
            "group"
        },
        user_id: user.id.0,
        username: user.username.as_deref(),
        first_name: &user.first_name,
        last_name: user.last_name.as_deref(),
        is_bot: user.is_bot,
        is_admin: None,
        status: "member",
    };
    store.upsert_member(&value).await?;
    Ok(())
}

pub async fn chat_member(update: ChatMemberUpdated, store: PgModerationStore) -> Result<()> {
    if !update.chat.is_group() && !update.chat.is_supergroup() {
        return Ok(());
    }
    let member = &update.new_chat_member;
    store
        .upsert_member(&MemberUpsert {
            chat_id: update.chat.id.0,
            chat_title: update.chat.title().unwrap_or("Telegram guruhi"),
            chat_username: update.chat.username(),
            chat_type: if update.chat.is_supergroup() {
                "supergroup"
            } else {
                "group"
            },
            user_id: member.user.id.0,
            username: member.user.username.as_deref(),
            first_name: &member.user.first_name,
            last_name: member.user.last_name.as_deref(),
            is_bot: member.user.is_bot,
            is_admin: Some(matches!(
                member.kind,
                ChatMemberKind::Owner(_) | ChatMemberKind::Administrator(_)
            )),
            status: indexed_member_status(&member.kind),
        })
        .await?;
    Ok(())
}

fn indexed_member_status(kind: &ChatMemberKind) -> &'static str {
    match kind {
        ChatMemberKind::Owner(_) => "creator",
        ChatMemberKind::Administrator(_) => "administrator",
        ChatMemberKind::Member(_) => "member",
        ChatMemberKind::Restricted(_) => "restricted",
        ChatMemberKind::Left => "left",
        ChatMemberKind::Banned(_) => "kicked",
    }
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

async fn moderate(
    bot: Bot,
    msg: Message,
    state: AppState,
    store: PgModerationStore,
    update_id: i64,
    check_flood: bool,
) -> Result<()> {
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
    let flood = check_flood
        && state.flood.observe(
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
        audit_delete(&store, &msg, user.id.0, update_id, "anti_flood").await;
        store
            .rich_audit(&RichAuditEvent {
                chat_id: msg.chat.id.0,
                actor_id: None,
                target_id: user.id.0,
                action: "anti_flood",
                reason: Some("sliding-window limit"),
                source: "auto",
                status: "success",
                duration_secs: None,
                telegram_message_id: Some(i64::from(msg.id.0)),
                telegram_update_id: Some(update_id),
                metadata: json!({"sanction":settings.flood_action.as_str()}),
            })
            .await?;
        store
            .create_incident(
                msg.chat.id.0,
                "anti_flood",
                "medium",
                json!({"user_id":user.id.0,"message_id":msg.id.0}),
            )
            .await?;
        punish(
            bot,
            msg,
            state,
            store,
            settings.flood_action,
            "anti-flood",
            update_id,
        )
        .await?;
        return Ok(());
    }

    let Some(text) = msg.text().or_else(|| msg.caption()) else {
        return Ok(());
    };
    let blocklist_enabled = store.module_enabled(msg.chat.id.0, "blocklist").await?;
    if blocklist_enabled
        && let ContentDecision::Block { matched_term } =
            ContentPolicy::evaluate(text, &blocked_terms)
    {
        bot.delete_message(msg.chat.id, msg.id).await?;
        audit_delete(&store, &msg, user.id.0, update_id, "blocklist").await;
        commands::warn_user(
            &bot,
            &state,
            &msg,
            user,
            None,
            &format!("blocklist: {matched_term}"),
            Some((&store, Some(update_id))),
        )
        .await?;
        store
            .record_blocklist_match(msg.chat.id.0, &matched_term)
            .await?;
        store
            .rich_audit(&RichAuditEvent {
                chat_id: msg.chat.id.0,
                actor_id: None,
                target_id: user.id.0,
                action: "blocklist_match",
                reason: Some(&matched_term),
                source: "auto",
                status: "success",
                duration_secs: None,
                telegram_message_id: Some(i64::from(msg.id.0)),
                telegram_update_id: Some(update_id),
                metadata: json!({"matched_term":matched_term}),
            })
            .await?;
        store
            .create_incident(
                msg.chat.id.0,
                "blocklist_match",
                "medium",
                json!({"user_id":user.id.0,"message_id":msg.id.0}),
            )
            .await?;
    }
    Ok(())
}

async fn audit_delete(
    store: &PgModerationStore,
    msg: &Message,
    target_id: u64,
    update_id: i64,
    reason: &str,
) {
    if let Err(error) = store
        .rich_audit(&RichAuditEvent {
            chat_id: msg.chat.id.0,
            actor_id: None,
            target_id,
            action: "delete",
            reason: Some(reason),
            source: "auto",
            status: "success",
            duration_secs: None,
            telegram_message_id: Some(i64::from(msg.id.0)),
            telegram_update_id: Some(update_id),
            metadata: json!({}),
        })
        .await
    {
        tracing::warn!(%error, "delete audit yozilmadi");
    }
}

async fn punish(
    bot: Bot,
    msg: Message,
    state: AppState,
    store: PgModerationStore,
    sanction: Sanction,
    reason: &str,
    update_id: i64,
) -> Result<()> {
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };
    match sanction {
        Sanction::Delete => {}
        Sanction::Warn => {
            commands::warn_user(
                &bot,
                &state,
                &msg,
                user,
                None,
                reason,
                Some((&store, Some(update_id))),
            )
            .await?
        }
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
            if let Some(status) = commands::sanction_member_status(sanction) {
                store
                    .update_member_status(msg.chat.id.0, user.id.0, status)
                    .await?;
            }
            bot.send_message(
                msg.chat.id,
                format!("{}: {} ({reason}).", user.first_name, sanction.as_str()),
            )
            .await?;
            if let Err(error) = store
                .rich_audit(&RichAuditEvent {
                    chat_id: msg.chat.id.0,
                    actor_id: None,
                    target_id: user.id.0,
                    action: sanction.as_str(),
                    reason: Some(reason),
                    source: "auto",
                    status: "success",
                    duration_secs: matches!(sanction, Sanction::Mute)
                        .then_some(settings.mute_duration_secs),
                    telegram_message_id: Some(i64::from(msg.id.0)),
                    telegram_update_id: Some(update_id),
                    metadata: json!({}),
                })
                .await
            {
                tracing::warn!(%error, "auto moderation audit yozilmadi");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod member_index_tests {
    use super::*;
    use serde_json::json;
    use teloxide::types::ChatMember;

    fn member(status: &str) -> ChatMember {
        serde_json::from_value(json!({
            "user": { "id": 42, "is_bot": false, "first_name": "Ali" },
            "status": status,
            "until_date": 0
        }))
        .expect("valid Telegram chat member fixture")
    }

    #[test]
    fn maps_chat_member_updates_to_index_statuses() {
        assert_eq!(indexed_member_status(&member("member").kind), "member");
        assert_eq!(indexed_member_status(&member("left").kind), "left");
        assert_eq!(indexed_member_status(&member("kicked").kind), "kicked");
    }
}
