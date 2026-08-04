use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type ChatId = i64;
pub type UserId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sanction {
    Delete,
    Warn,
    Mute,
    Ban,
}

impl Sanction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Warn => "warn",
            Self::Mute => "mute",
            Self::Ban => "ban",
        }
    }
}

impl std::str::FromStr for Sanction {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "delete" | "ochir" | "o'chir" => Ok(Self::Delete),
            "warn" | "ogohlantir" => Ok(Self::Warn),
            "mute" => Ok(Self::Mute),
            "ban" => Ok(Self::Ban),
            _ => Err("action: delete, warn, mute yoki ban bo'lishi kerak"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSettings {
    pub chat_id: ChatId,
    pub flood_limit: u16,
    pub flood_window_secs: u64,
    pub flood_action: Sanction,
    pub warn_limit: u16,
    pub warn_action: Sanction,
    pub mute_duration_secs: u64,
    pub welcome_enabled: bool,
    pub welcome_template: String,
    pub rules: String,
}

impl ChatSettings {
    pub fn defaults(chat_id: ChatId) -> Self {
        Self {
            chat_id,
            flood_limit: 8,
            flood_window_secs: 10,
            flood_action: Sanction::Mute,
            warn_limit: 3,
            warn_action: Sanction::Mute,
            mute_duration_secs: 3600,
            welcome_enabled: true,
            welcome_template:
                "Xush kelibsiz, {first_name}! {chat_title} qoidalarini /rules orqali o'qing.".into(),
            rules: "Guruh qoidalari hali administrator tomonidan kiritilmagan.".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningState {
    pub count: u16,
    pub limit: u16,
    pub reached_limit: bool,
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub chat_id: ChatId,
    pub actor_id: Option<UserId>,
    pub target_id: UserId,
    pub action: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}
