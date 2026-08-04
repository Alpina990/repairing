use async_trait::async_trait;
use thiserror::Error;

use crate::{AuditEvent, ChatId, ChatSettings, Sanction, UserId, WarningState};

#[derive(Debug, Error)]
#[error("storage operation failed: {message}")]
pub struct StoreError {
    pub message: String,
}

impl StoreError {
    pub fn new(error: impl std::fmt::Display) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

#[async_trait]
pub trait ModerationStore: Send + Sync {
    async fn healthcheck(&self) -> Result<(), StoreError>;
    async fn settings(&self, chat_id: ChatId) -> Result<ChatSettings, StoreError>;
    async fn set_flood(
        &self,
        chat_id: ChatId,
        limit: u16,
        window_secs: u64,
        action: Sanction,
    ) -> Result<(), StoreError>;
    async fn set_warn_limit(&self, chat_id: ChatId, limit: u16) -> Result<(), StoreError>;
    async fn set_welcome(
        &self,
        chat_id: ChatId,
        enabled: bool,
        template: Option<&str>,
    ) -> Result<(), StoreError>;
    async fn set_rules(&self, chat_id: ChatId, rules: &str) -> Result<(), StoreError>;
    async fn blocked_terms(&self, chat_id: ChatId) -> Result<Vec<String>, StoreError>;
    async fn add_blocked_term(&self, chat_id: ChatId, term: &str) -> Result<bool, StoreError>;
    async fn remove_blocked_term(&self, chat_id: ChatId, term: &str) -> Result<bool, StoreError>;
    async fn add_warning(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        actor_id: Option<UserId>,
        reason: &str,
    ) -> Result<WarningState, StoreError>;
    async fn warning_count(&self, chat_id: ChatId, user_id: UserId) -> Result<u16, StoreError>;
    async fn clear_warnings(&self, chat_id: ChatId, user_id: UserId) -> Result<u64, StoreError>;
    async fn audit(&self, event: &AuditEvent) -> Result<(), StoreError>;
}
