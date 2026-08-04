use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use dashmap::DashMap;
use qalqon_core::{ChatSettings, FloodGuard, ModerationStore};
use teloxide::{
    Bot,
    prelude::Requester,
    types::{ChatId, UserId},
};

type CachedPolicy = CacheEntry<(ChatSettings, Vec<String>)>;
type CachedAdmins = CacheEntry<HashSet<u64>>;
const MAX_ADMIN_CACHE_ENTRIES: usize = 20_000;
const MAX_POLICY_CACHE_ENTRIES: usize = 20_000;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ModerationStore>,
    pub flood: Arc<FloodGuard>,
    owners: Arc<HashSet<u64>>,
    admin_cache: Arc<DashMap<i64, CachedAdmins>>,
    policy_cache: Arc<DashMap<i64, CachedPolicy>>,
    admin_ttl: Duration,
    policy_ttl: Duration,
}

#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl AppState {
    pub fn new(
        store: Arc<dyn ModerationStore>,
        owners: HashSet<u64>,
        admin_ttl: Duration,
        policy_ttl: Duration,
    ) -> Self {
        Self {
            store,
            flood: Arc::new(FloodGuard::default()),
            owners: Arc::new(owners),
            admin_cache: Arc::new(DashMap::new()),
            policy_cache: Arc::new(DashMap::new()),
            admin_ttl,
            policy_ttl,
        }
    }

    pub async fn is_admin(
        &self,
        bot: &Bot,
        chat_id: ChatId,
        user_id: UserId,
        fresh: bool,
    ) -> Result<bool> {
        if self.owners.contains(&user_id.0) {
            return Ok(true);
        }

        if fresh {
            return Ok(bot.get_chat_member(chat_id, user_id).await?.is_privileged());
        }

        if let Some(entry) = self.admin_cache.get(&chat_id.0) {
            if entry.expires_at > Instant::now() {
                return Ok(entry.value.contains(&user_id.0));
            }
        }

        let admins = bot
            .get_chat_administrators(chat_id)
            .await?
            .into_iter()
            .map(|member| member.user.id.0)
            .collect::<HashSet<_>>();
        let privileged = admins.contains(&user_id.0);
        let now = Instant::now();
        if self.admin_cache.len() >= MAX_ADMIN_CACHE_ENTRIES {
            self.admin_cache.retain(|_, entry| entry.expires_at > now);
        }
        if self.admin_cache.len() < MAX_ADMIN_CACHE_ENTRIES {
            self.admin_cache.insert(
                chat_id.0,
                CacheEntry {
                    value: admins,
                    expires_at: now + self.admin_ttl,
                },
            );
        }
        Ok(privileged)
    }

    pub async fn policy(&self, chat_id: i64) -> Result<(ChatSettings, Vec<String>)> {
        if let Some(entry) = self.policy_cache.get(&chat_id) {
            if entry.expires_at > Instant::now() {
                return Ok(entry.value.clone());
            }
        }

        let settings = self.store.settings(chat_id).await?;
        let terms = self.store.blocked_terms(chat_id).await?;
        let value = (settings, terms);
        let now = Instant::now();
        if self.policy_cache.len() >= MAX_POLICY_CACHE_ENTRIES {
            self.policy_cache.retain(|_, entry| entry.expires_at > now);
        }
        if self.policy_cache.len() < MAX_POLICY_CACHE_ENTRIES {
            self.policy_cache.insert(
                chat_id,
                CacheEntry {
                    value: value.clone(),
                    expires_at: now + self.policy_ttl,
                },
            );
        }
        Ok(value)
    }

    pub fn invalidate_policy(&self, chat_id: i64) {
        self.policy_cache.remove(&chat_id);
    }
}
