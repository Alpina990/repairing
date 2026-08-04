use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use dashmap::DashMap;

use crate::{ChatId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloodKey {
    pub chat_id: ChatId,
    pub user_id: UserId,
}

/// Har bir (chat, user) uchun sliding-window anti-flood hisoblagichi.
#[derive(Debug, Default)]
pub struct FloodGuard {
    events: DashMap<FloodKey, VecDeque<Instant>>,
}

impl FloodGuard {
    pub fn observe(&self, key: FloodKey, limit: u16, window: Duration, now: Instant) -> bool {
        if limit == 0 {
            return false;
        }

        let cutoff = now.checked_sub(window).unwrap_or(now);
        let mut queue = self.events.entry(key).or_default();
        while queue.front().is_some_and(|timestamp| *timestamp < cutoff) {
            queue.pop_front();
        }
        queue.push_back(now);

        if queue.len() >= usize::from(limit) {
            queue.clear();
            true
        } else {
            false
        }
    }

    pub fn forget(&self, key: FloodKey) {
        self.events.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_at_limit_and_resets() {
        let guard = FloodGuard::default();
        let key = FloodKey {
            chat_id: -1,
            user_id: 7,
        };
        let now = Instant::now();

        assert!(!guard.observe(key, 3, Duration::from_secs(10), now));
        assert!(!guard.observe(
            key,
            3,
            Duration::from_secs(10),
            now + Duration::from_secs(1)
        ));
        assert!(guard.observe(
            key,
            3,
            Duration::from_secs(10),
            now + Duration::from_secs(2)
        ));
        assert!(!guard.observe(
            key,
            3,
            Duration::from_secs(10),
            now + Duration::from_secs(3)
        ));
    }

    #[test]
    fn expires_old_events() {
        let guard = FloodGuard::default();
        let key = FloodKey {
            chat_id: -1,
            user_id: 8,
        };
        let now = Instant::now();

        assert!(!guard.observe(key, 2, Duration::from_secs(3), now));
        assert!(!guard.observe(key, 2, Duration::from_secs(3), now + Duration::from_secs(4)));
    }
}
