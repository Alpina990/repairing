use std::str::FromStr;

use anyhow::{Context, Result, bail};
use chrono::{Duration as ChronoDuration, Utc};
use qalqon_core::{AuditEvent, ChatSettings, ModerationStore, Sanction};
use qalqon_storage::{PgModerationStore, RichAuditEvent};
use serde_json::json;
use teloxide::{
    Bot,
    payloads::{RestrictChatMemberSetters, SendMessageSetters, UnbanChatMemberSetters},
    prelude::Requester,
    types::{
        ChatId, ChatPermissions, InlineKeyboardButton, InlineKeyboardMarkup, Message, User, UserId,
        WebAppInfo,
    },
    utils::command::BotCommands,
};

use crate::state::AppState;

#[derive(BotCommands, Clone, Copy, Debug)]
#[command(rename_rule = "lowercase", description = "CheklaBot buyruqlari:")]
pub enum Command {
    #[command(description = "botni ishga tushirish")]
    Start,
    #[command(description = "buyruqlar ro'yxati")]
    Help,
    #[command(description = "guruh moderatsiya sozlamalari")]
    Settings,
    #[command(description = "guruh qoidalarini ko'rish")]
    Rules,
    #[command(description = "qoidalarni o'rnatish: /setrules matn")]
    Setrules,
    #[command(description = "reply qilingan a'zoni ogohlantirish")]
    Warn,
    #[command(description = "reply qilingan a'zoning warninglarini tozalash")]
    Unwarn,
    #[command(description = "reply qilingan a'zoning warning soni")]
    Warnings,
    #[command(description = "reply qilingan a'zoni ban qilish")]
    Ban,
    #[command(description = "ID bo'yicha bandan chiqarish: /unban 123")]
    Unban,
    #[command(description = "reply qilingan a'zoni mute qilish")]
    Mute,
    #[command(description = "reply qilingan a'zoni unmute qilish")]
    Unmute,
    #[command(description = "anti-flood: /setflood limit window action")]
    Setflood,
    #[command(description = "warning limit: /setwarnlimit 3")]
    Setwarnlimit,
    #[command(description = "welcome holati: /welcome on|off")]
    Welcome,
    #[command(description = "welcome matni: /setwelcome matn")]
    Setwelcome,
    #[command(description = "taqiqlangan ibora qo'shish")]
    Addblock,
    #[command(description = "taqiqlangan iborani o'chirish")]
    Rmblock,
    #[command(description = "taqiqlangan iboralar ro'yxati")]
    Blocklist,
}

pub async fn handle(
    bot: Bot,
    msg: Message,
    command: Command,
    state: AppState,
    store: PgModerationStore,
    mini_app: Option<WebAppInfo>,
) -> Result<()> {
    let chat_id = msg.chat.id;
    match command {
        Command::Start => {
            let request = bot.send_message(chat_id, "CheklaBot ishlayapti. Guruhga administrator qilib qo'shing va /help buyrug'ini yuboring.");
            let request = match start_reply_markup(msg.chat.is_private(), mini_app) {
                Some(markup) => request.reply_markup(markup),
                None => request,
            };
            request.await?;
        }
        Command::Help => {
            bot.send_message(chat_id, Command::descriptions().to_string())
                .await?;
        }
        Command::Rules => {
            ensure_group(&msg)?;
            bot.send_message(chat_id, state.store.settings(chat_id.0).await?.rules)
                .await?;
        }
        Command::Settings => {
            ensure_group(&msg)?;
            let settings = state.store.settings(chat_id.0).await?;
            bot.send_message(chat_id, format_settings(&settings))
                .await?;
        }
        Command::Blocklist => {
            ensure_group(&msg)?;
            let terms = state.store.blocked_terms(chat_id.0).await?;
            let text = if terms.is_empty() {
                "Blocklist bo'sh.".into()
            } else {
                format!("Blocklist:\n• {}", terms.join("\n• "))
            };
            bot.send_message(chat_id, text).await?;
        }
        Command::Setrules => {
            require_admin(&bot, &msg, &state).await?;
            let rules = require_args(&msg, "/setrules <matn>")?;
            if rules.chars().count() > 3500 {
                bail!("qoidalar 3500 belgidan oshmasligi kerak");
            }
            state.store.set_rules(chat_id.0, rules).await?;
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                "rules_update",
                Some(rules),
                None,
            )
            .await;
            bot.send_message(chat_id, "Qoidalar saqlandi.").await?;
        }
        Command::Setflood => {
            require_admin(&bot, &msg, &state).await?;
            let (limit, window, action) = parse_flood(require_args(
                &msg,
                "/setflood <limit> [sekund] [delete|warn|mute|ban]",
            )?)?;
            state
                .store
                .set_flood(chat_id.0, limit, window, action)
                .await?;
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                "settings_update",
                Some("anti_flood"),
                None,
            )
            .await;
            bot.send_message(
                chat_id,
                format!(
                    "Anti-flood: {limit} xabar / {window}s → {}",
                    action.as_str()
                ),
            )
            .await?;
        }
        Command::Setwarnlimit => {
            require_admin(&bot, &msg, &state).await?;
            let limit = require_args(&msg, "/setwarnlimit <1..100>")?.parse::<u16>()?;
            if !(1..=100).contains(&limit) {
                bail!("warning limit 1..100 oralig'ida bo'lishi kerak");
            }
            state.store.set_warn_limit(chat_id.0, limit).await?;
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                "settings_update",
                Some("warn_limit"),
                None,
            )
            .await;
            bot.send_message(chat_id, format!("Warning limiti: {limit}"))
                .await?;
        }
        Command::Welcome => {
            require_admin(&bot, &msg, &state).await?;
            let enabled = parse_on_off(require_args(&msg, "/welcome on|off")?)?;
            state.store.set_welcome(chat_id.0, enabled, None).await?;
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                "welcome_update",
                Some(if enabled { "on" } else { "off" }),
                None,
            )
            .await;
            bot.send_message(
                chat_id,
                if enabled {
                    "Welcome yoqildi."
                } else {
                    "Welcome o'chirildi."
                },
            )
            .await?;
        }
        Command::Setwelcome => {
            require_admin(&bot, &msg, &state).await?;
            let template = require_args(&msg, "/setwelcome <matn>")?;
            if template.len() > 3500 {
                bail!("welcome matni 3500 belgidan oshmasligi kerak");
            }
            state
                .store
                .set_welcome(chat_id.0, true, Some(template))
                .await?;
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                "welcome_update",
                Some(template),
                None,
            )
            .await;
            bot.send_message(chat_id, "Welcome matni saqlandi. Placeholderlar: {first_name}, {username}, {user_id}, {chat_title}").await?;
        }
        Command::Addblock | Command::Rmblock => {
            require_admin(&bot, &msg, &state).await?;
            let term = require_args(&msg, "/addblock yoki /rmblock <ibora>")?;
            if term.chars().count() > 200 {
                bail!("ibora 200 belgidan oshmasligi kerak");
            }
            let changed = if matches!(command, Command::Addblock) {
                state.store.add_blocked_term(chat_id.0, term).await?
            } else {
                state.store.remove_blocked_term(chat_id.0, term).await?
            };
            state.invalidate_policy(chat_id.0);
            audit_command(
                &store,
                &msg,
                actor_id(&msg),
                if matches!(command, Command::Addblock) {
                    "blocklist_add"
                } else {
                    "blocklist_remove"
                },
                Some(term),
                None,
            )
            .await;
            bot.send_message(
                chat_id,
                if changed {
                    "Blocklist yangilandi."
                } else {
                    "O'zgarish bo'lmadi."
                },
            )
            .await?;
        }
        Command::Warn => {
            require_admin(&bot, &msg, &state).await?;
            let target = reply_target(&msg)?;
            let reason = args(&msg).unwrap_or("Sabab ko'rsatilmagan");
            warn_user(
                &bot,
                &state,
                &msg,
                target,
                msg.from.as_ref().map(|u| u.id.0),
                reason,
                Some((&store, None)),
            )
            .await?;
        }
        Command::Unwarn => {
            require_admin(&bot, &msg, &state).await?;
            let target = reply_target(&msg)?;
            let removed = state.store.clear_warnings(chat_id.0, target.id.0).await?;
            audit_command(&store, &msg, target.id.0, "unwarn", None, None).await;
            bot.send_message(chat_id, format!("{} ta warning olib tashlandi.", removed))
                .await?;
        }
        Command::Warnings => {
            ensure_group(&msg)?;
            let target = msg
                .reply_to_message()
                .and_then(|m| m.from.as_ref())
                .or(msg.from.as_ref())
                .context("foydalanuvchi topilmadi")?;
            let count = state.store.warning_count(chat_id.0, target.id.0).await?;
            bot.send_message(
                chat_id,
                format!("{}: {count} ta warning.", target.first_name),
            )
            .await?;
        }
        Command::Ban | Command::Mute | Command::Unmute => {
            require_admin(&bot, &msg, &state).await?;
            let target = reply_target(&msg)?;
            ensure_not_admin(&bot, chat_id, target).await?;
            match command {
                Command::Ban => {
                    execute_sanction(&bot, chat_id, target.id, Sanction::Ban, 0).await?
                }
                Command::Mute => {
                    let duration = parse_duration(args(&msg).unwrap_or("1h"))?;
                    execute_sanction(&bot, chat_id, target.id, Sanction::Mute, duration).await?;
                }
                Command::Unmute => {
                    bot.restrict_chat_member(chat_id, target.id, ChatPermissions::all())
                        .await?;
                }
                _ => unreachable!(),
            }
            let member_status = match command {
                Command::Ban => "kicked",
                Command::Mute => "restricted",
                Command::Unmute => "member",
                _ => unreachable!(),
            };
            store
                .update_member_status(chat_id.0, target.id.0, member_status)
                .await?;
            audit_command(
                &store,
                &msg,
                target.id.0,
                match command {
                    Command::Ban => "ban",
                    Command::Mute => "mute",
                    Command::Unmute => "unmute",
                    _ => unreachable!(),
                },
                args(&msg),
                matches!(command, Command::Mute)
                    .then(|| parse_duration(args(&msg).unwrap_or("1h")).ok())
                    .flatten(),
            )
            .await;
            bot.send_message(chat_id, "Bajarildi.").await?;
        }
        Command::Unban => {
            require_admin(&bot, &msg, &state).await?;
            let id = require_args(&msg, "/unban <user_id>")?.parse::<u64>()?;
            bot.unban_chat_member(chat_id, UserId(id))
                .only_if_banned(true)
                .await?;
            store.update_member_status(chat_id.0, id, "left").await?;
            audit_command(&store, &msg, id, "unban", None, None).await;
            bot.send_message(chat_id, "Foydalanuvchi bandan chiqarildi.")
                .await?;
        }
    }
    Ok(())
}

fn start_reply_markup(
    is_private_chat: bool,
    mini_app: Option<WebAppInfo>,
) -> Option<InlineKeyboardMarkup> {
    if !is_private_chat {
        return None;
    }
    mini_app.map(|web_app| {
        InlineKeyboardMarkup::new([[InlineKeyboardButton::web_app("Mini Appni ochish", web_app)]])
    })
}

pub async fn warn_user(
    bot: &Bot,
    state: &AppState,
    msg: &Message,
    target: &User,
    actor_id: Option<u64>,
    reason: &str,
    audit: Option<(&PgModerationStore, Option<i64>)>,
) -> Result<()> {
    if reason.chars().count() > 1000 {
        bail!("warning sababi 1000 belgidan oshmasligi kerak");
    }
    ensure_not_admin(bot, msg.chat.id, target).await?;
    let warning = state
        .store
        .add_warning(msg.chat.id.0, target.id.0, actor_id, reason)
        .await?;
    if warning.reached_limit {
        let settings = state.store.settings(msg.chat.id.0).await?;
        execute_sanction(
            bot,
            msg.chat.id,
            target.id,
            settings.warn_action,
            settings.mute_duration_secs,
        )
        .await?;
        if let (Some(status), Some((store, _))) =
            (sanction_member_status(settings.warn_action), audit)
        {
            store
                .update_member_status(msg.chat.id.0, target.id.0, status)
                .await?;
        }
        state
            .store
            .clear_warnings(msg.chat.id.0, target.id.0)
            .await?;
        bot.send_message(
            msg.chat.id,
            format!(
                "{} warning limitiga yetdi ({}/{}): {}.",
                target.first_name,
                warning.count,
                warning.limit,
                settings.warn_action.as_str()
            ),
        )
        .await?;
    } else {
        bot.send_message(
            msg.chat.id,
            format!(
                "{} ogohlantirildi ({}/{}). Sabab: {}",
                target.first_name, warning.count, warning.limit, reason
            ),
        )
        .await?;
    }
    if let Some((store, telegram_update_id)) = audit {
        if let Err(error) = store
            .rich_audit(&RichAuditEvent {
                chat_id: msg.chat.id.0,
                actor_id,
                target_id: target.id.0,
                action: "warn",
                reason: Some(reason),
                source: if actor_id.is_some() { "admin" } else { "auto" },
                status: "success",
                duration_secs: None,
                telegram_message_id: Some(i64::from(msg.id.0)),
                telegram_update_id,
                metadata: json!({"count":warning.count,"limit":warning.limit,"reached_limit":warning.reached_limit}),
            })
            .await
        {
            tracing::warn!(%error, "warning audit yozilmadi");
        }
    } else {
        audit_best_effort(
            state.store.as_ref(),
            msg.chat.id.0,
            actor_id,
            target.id.0,
            "warn",
            Some(reason),
        )
        .await;
    }
    Ok(())
}

fn actor_id(msg: &Message) -> u64 {
    msg.from.as_ref().map_or(0, |user| user.id.0)
}

async fn audit_command(
    store: &PgModerationStore,
    msg: &Message,
    target_id: u64,
    action: &str,
    reason: Option<&str>,
    duration_secs: Option<u64>,
) {
    if let Err(error) = store
        .rich_audit(&RichAuditEvent {
            chat_id: msg.chat.id.0,
            actor_id: msg.from.as_ref().map(|user| user.id.0),
            target_id,
            action,
            reason,
            source: "admin",
            status: "success",
            duration_secs,
            telegram_message_id: Some(i64::from(msg.id.0)),
            telegram_update_id: None,
            metadata: json!({}),
        })
        .await
    {
        tracing::warn!(%error, "command audit yozilmadi");
    }
}

pub async fn execute_sanction(
    bot: &Bot,
    chat_id: ChatId,
    user_id: UserId,
    sanction: Sanction,
    duration_secs: u64,
) -> Result<()> {
    match sanction {
        Sanction::Delete | Sanction::Warn => {}
        Sanction::Mute => {
            let until = Utc::now() + ChronoDuration::seconds(i64::try_from(duration_secs.max(30))?);
            bot.restrict_chat_member(chat_id, user_id, ChatPermissions::empty())
                .until_date(until)
                .await?;
        }
        Sanction::Ban => {
            bot.ban_chat_member(chat_id, user_id).await?;
        }
    }
    Ok(())
}

pub(crate) fn sanction_member_status(sanction: Sanction) -> Option<&'static str> {
    match sanction {
        Sanction::Mute => Some("restricted"),
        Sanction::Ban => Some("kicked"),
        Sanction::Delete | Sanction::Warn => None,
    }
}

pub async fn audit_best_effort(
    store: &dyn ModerationStore,
    chat_id: i64,
    actor_id: Option<u64>,
    target_id: u64,
    action: &str,
    reason: Option<&str>,
) {
    let event = AuditEvent {
        chat_id,
        actor_id,
        target_id,
        action: action.into(),
        reason: reason.map(str::to_owned),
        created_at: Utc::now(),
    };
    if let Err(error) = store.audit(&event).await {
        tracing::warn!(%error, "audit event yozilmadi");
    }
}

async fn require_admin(bot: &Bot, msg: &Message, state: &AppState) -> Result<()> {
    ensure_group(msg)?;
    let user = msg
        .from
        .as_ref()
        .context("anonymous admin komandasi qo'llab-quvvatlanmaydi")?;
    if !state.is_admin(bot, msg.chat.id, user.id, true).await? {
        bail!("bu buyruq faqat guruh administratorlari uchun");
    }
    Ok(())
}

async fn ensure_not_admin(bot: &Bot, chat_id: ChatId, target: &User) -> Result<()> {
    if target.is_bot {
        bail!("bot yoki administratorga bu amalni bajarib bo'lmaydi");
    }
    if bot
        .get_chat_member(chat_id, target.id)
        .await?
        .is_privileged()
    {
        bail!("administratorga bu amalni bajarib bo'lmaydi");
    }
    Ok(())
}

fn ensure_group(msg: &Message) -> Result<()> {
    if !msg.chat.is_group() && !msg.chat.is_supergroup() {
        bail!("bu buyruq faqat guruhda ishlaydi");
    }
    Ok(())
}

fn reply_target(msg: &Message) -> Result<&User> {
    msg.reply_to_message()
        .and_then(|reply| reply.from.as_ref())
        .context("buyruqni foydalanuvchi xabariga reply qilib yuboring")
}

fn args(msg: &Message) -> Option<&str> {
    msg.text()?
        .split_once(char::is_whitespace)
        .map(|(_, args)| args.trim())
        .filter(|args| !args.is_empty())
}

fn require_args<'a>(msg: &'a Message, usage: &str) -> Result<&'a str> {
    args(msg).with_context(|| format!("foydalanish: {usage}"))
}

fn parse_flood(value: &str) -> Result<(u16, u64, Sanction)> {
    let mut values = value.split_whitespace();
    let limit = values.next().context("limit yo'q")?.parse::<u16>()?;
    let window = values.next().unwrap_or("10").parse::<u64>()?;
    let action = Sanction::from_str(values.next().unwrap_or("mute")).map_err(anyhow::Error::msg)?;
    if limit != 0 && !(3..=100).contains(&limit) {
        bail!("flood limit 0 yoki 3..100 bo'lishi kerak");
    }
    if !(1..=3600).contains(&window) {
        bail!("window 1..3600 sekund bo'lishi kerak");
    }
    Ok((limit, window, action))
}

fn parse_on_off(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "yes" | "1" | "yoq" => Ok(true),
        "off" | "no" | "0" | "ochir" | "o'chir" => Ok(false),
        _ => bail!("on yoki off kiriting"),
    }
}

fn parse_duration(value: &str) -> Result<u64> {
    let value = value.trim().to_ascii_lowercase();
    let (number, multiplier) = match value.chars().last() {
        Some('m') => (&value[..value.len() - 1], 60),
        Some('h') => (&value[..value.len() - 1], 3600),
        Some('d') => (&value[..value.len() - 1], 86400),
        Some('w') => (&value[..value.len() - 1], 604800),
        _ => (value.as_str(), 1),
    };
    let seconds = number
        .parse::<u64>()?
        .checked_mul(multiplier)
        .context("duration juda katta")?;
    if !(30..=31_536_000).contains(&seconds) {
        bail!("duration 30 sekunddan 365 kungacha bo'lishi kerak");
    }
    Ok(seconds)
}

fn format_settings(value: &ChatSettings) -> String {
    format!(
        "CheklaBot sozlamalari\n\nAnti-flood: {} xabar / {}s → {}\nWarning: {} ta → {}\nMute: {}s\nWelcome: {}\nBlocklist: /blocklist",
        value.flood_limit,
        value.flood_window_secs,
        value.flood_action.as_str(),
        value.warn_limit,
        value.warn_action.as_str(),
        value.mute_duration_secs,
        if value.welcome_enabled { "on" } else { "off" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo};
    use url::Url;

    #[test]
    fn start_reply_markup_exposes_mini_app_in_private_chat() {
        let web_app = WebAppInfo {
            url: Url::parse("https://mini.example.com").unwrap(),
        };

        assert_eq!(
            start_reply_markup(true, Some(web_app.clone())),
            Some(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::web_app("Mini Appni ochish", web_app)
            ]]))
        );
    }

    #[test]
    fn start_reply_markup_is_not_added_to_groups_or_without_url() {
        let web_app = WebAppInfo {
            url: Url::parse("https://mini.example.com").unwrap(),
        };

        assert_eq!(start_reply_markup(false, Some(web_app)), None);
        assert_eq!(start_reply_markup(true, None), None);
    }

    #[test]
    fn parses_flood_defaults() {
        assert_eq!(parse_flood("8").unwrap(), (8, 10, Sanction::Mute));
        assert_eq!(
            parse_flood("0 15 delete").unwrap(),
            (0, 15, Sanction::Delete)
        );
    }

    #[test]
    fn parses_human_duration() {
        assert_eq!(parse_duration("30m").unwrap(), 1800);
        assert_eq!(parse_duration("2h").unwrap(), 7200);
    }

    #[test]
    fn maps_sanctions_to_member_index_statuses() {
        assert_eq!(sanction_member_status(Sanction::Mute), Some("restricted"));
        assert_eq!(sanction_member_status(Sanction::Ban), Some("kicked"));
        assert_eq!(sanction_member_status(Sanction::Warn), None);
        assert_eq!(sanction_member_status(Sanction::Delete), None);
    }
}
