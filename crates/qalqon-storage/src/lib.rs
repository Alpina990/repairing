use std::time::Duration;

use async_trait::async_trait;
use qalqon_core::{
    AuditEvent, ChatId, ChatSettings, ModerationStore, Sanction, StoreError, UserId, WarningState,
};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

pub mod admin;
pub use admin::*;

#[derive(Debug, Clone)]
pub struct PgModerationStore {
    pool: PgPool,
}

impl PgModerationStore {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    async fn ensure_chat(&self, chat_id: ChatId) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO chat_settings (chat_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(chat_id)
            .execute(&self.pool)
            .await
            .map_err(StoreError::new)?;
        Ok(())
    }
}

fn parse_sanction(value: &str) -> Result<Sanction, StoreError> {
    value.parse().map_err(StoreError::new)
}

fn user_as_i64(user_id: UserId) -> Result<i64, StoreError> {
    i64::try_from(user_id).map_err(StoreError::new)
}

#[async_trait]
impl ModerationStore for PgModerationStore {
    async fn healthcheck(&self) -> Result<(), StoreError> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(StoreError::new)?;
        Ok(())
    }

    async fn settings(&self, chat_id: ChatId) -> Result<ChatSettings, StoreError> {
        self.ensure_chat(chat_id).await?;
        let row = sqlx::query(
            "SELECT chat_id, flood_limit, flood_window_secs, flood_action, warn_limit, warn_action, mute_duration_secs, welcome_enabled, welcome_template, rules FROM chat_settings WHERE chat_id = $1",
        )
        .bind(chat_id)
        .fetch_one(&self.pool)
        .await
        .map_err(StoreError::new)?;

        Ok(ChatSettings {
            chat_id: row.get("chat_id"),
            flood_limit: u16::try_from(row.get::<i32, _>("flood_limit"))
                .map_err(StoreError::new)?,
            flood_window_secs: u64::try_from(row.get::<i32, _>("flood_window_secs"))
                .map_err(StoreError::new)?,
            flood_action: parse_sanction(row.get("flood_action"))?,
            warn_limit: u16::try_from(row.get::<i32, _>("warn_limit")).map_err(StoreError::new)?,
            warn_action: parse_sanction(row.get("warn_action"))?,
            mute_duration_secs: u64::try_from(row.get::<i32, _>("mute_duration_secs"))
                .map_err(StoreError::new)?,
            welcome_enabled: row.get("welcome_enabled"),
            welcome_template: row.get("welcome_template"),
            rules: row.get("rules"),
        })
    }

    async fn set_flood(
        &self,
        chat_id: ChatId,
        limit: u16,
        window_secs: u64,
        action: Sanction,
    ) -> Result<(), StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query("UPDATE chat_settings SET flood_limit=$2, flood_window_secs=$3, flood_action=$4, updated_at=NOW() WHERE chat_id=$1")
            .bind(chat_id).bind(i32::from(limit)).bind(i32::try_from(window_secs).map_err(StoreError::new)?)
            .bind(action.as_str()).execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET enabled=$2>0,configured=TRUE,config=jsonb_build_object('limit',$2,'window_secs',$3,'action',$4),updated_at=NOW() WHERE chat_id=$1 AND module_key='anti_flood'")
            .bind(chat_id).bind(i32::from(limit)).bind(i32::try_from(window_secs).map_err(StoreError::new)?)
            .bind(action.as_str()).execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }

    async fn set_warn_limit(&self, chat_id: ChatId, limit: u16) -> Result<(), StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query("UPDATE chat_settings SET warn_limit=$2, updated_at=NOW() WHERE chat_id=$1")
            .bind(chat_id)
            .bind(i32::from(limit))
            .execute(&self.pool)
            .await
            .map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET enabled=TRUE,configured=TRUE,config=jsonb_set(config,'{limit}',to_jsonb($2::INTEGER),TRUE),updated_at=NOW() WHERE chat_id=$1 AND module_key='warning_policy'")
            .bind(chat_id).bind(i32::from(limit)).execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }

    async fn set_welcome(
        &self,
        chat_id: ChatId,
        enabled: bool,
        template: Option<&str>,
    ) -> Result<(), StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query("UPDATE chat_settings SET welcome_enabled=$2, welcome_template=COALESCE($3, welcome_template), updated_at=NOW() WHERE chat_id=$1")
            .bind(chat_id).bind(enabled).bind(template).execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET enabled=$2,configured=CASE WHEN $3::TEXT IS NULL THEN configured ELSE $3<>'' END,config=CASE WHEN $3::TEXT IS NULL THEN config ELSE jsonb_build_object('template',$3::TEXT) END,updated_at=NOW() WHERE chat_id=$1 AND module_key='welcome'")
            .bind(chat_id).bind(enabled).bind(template).execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }

    async fn set_rules(&self, chat_id: ChatId, rules: &str) -> Result<(), StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query("UPDATE chat_settings SET rules=$2, updated_at=NOW() WHERE chat_id=$1")
            .bind(chat_id)
            .bind(rules)
            .execute(&self.pool)
            .await
            .map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET enabled=trim($2)<>'',configured=trim($2)<>'',config=jsonb_build_object('rules',$2),updated_at=NOW() WHERE chat_id=$1 AND module_key='rules'")
            .bind(chat_id).bind(rules).execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }

    async fn blocked_terms(&self, chat_id: ChatId) -> Result<Vec<String>, StoreError> {
        let rows = sqlx::query("SELECT term FROM blocked_terms WHERE chat_id=$1 ORDER BY term")
            .bind(chat_id)
            .fetch_all(&self.pool)
            .await
            .map_err(StoreError::new)?;
        Ok(rows.into_iter().map(|row| row.get("term")).collect())
    }

    async fn add_blocked_term(&self, chat_id: ChatId, term: &str) -> Result<bool, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        let result = sqlx::query(
            "INSERT INTO blocked_terms(chat_id, term) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(chat_id)
        .bind(term.trim())
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        if result.rows_affected() == 1 {
            sqlx::query("UPDATE protection_modules SET enabled=TRUE,configured=TRUE,updated_at=NOW() WHERE chat_id=$1 AND module_key='blocklist'")
                .bind(chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        }
        Ok(result.rows_affected() == 1)
    }

    async fn remove_blocked_term(&self, chat_id: ChatId, term: &str) -> Result<bool, StoreError> {
        let result =
            sqlx::query("DELETE FROM blocked_terms WHERE chat_id=$1 AND lower(term)=lower($2)")
                .bind(chat_id)
                .bind(term.trim())
                .execute(&self.pool)
                .await
                .map_err(StoreError::new)?;
        if result.rows_affected() > 0 {
            sqlx::query("UPDATE protection_modules SET configured=EXISTS(SELECT 1 FROM blocked_terms WHERE chat_id=$1),updated_at=NOW() WHERE chat_id=$1 AND module_key='blocklist'")
                .bind(chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        }
        Ok(result.rows_affected() > 0)
    }

    async fn add_warning(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        actor_id: Option<UserId>,
        reason: &str,
    ) -> Result<WarningState, StoreError> {
        self.ensure_chat(chat_id).await?;
        let user_id = user_as_i64(user_id)?;
        let actor_id = actor_id.map(user_as_i64).transpose()?;
        let mut tx = self.pool.begin().await.map_err(StoreError::new)?;
        // Parallel warninglar bir-birining count natijasini o'tkazib yubormasin.
        let lock_key = format!("{chat_id}:{user_id}");
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::new)?;
        sqlx::query("INSERT INTO warnings(chat_id,user_id,actor_id,reason) VALUES($1,$2,$3,$4)")
            .bind(chat_id)
            .bind(user_id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::new)?;
        let row = sqlx::query("SELECT COUNT(*)::BIGINT AS count, s.warn_limit FROM warnings w JOIN chat_settings s ON s.chat_id=w.chat_id WHERE w.chat_id=$1 AND w.user_id=$2 GROUP BY s.warn_limit")
            .bind(chat_id).bind(user_id).fetch_one(&mut *tx).await.map_err(StoreError::new)?;
        tx.commit().await.map_err(StoreError::new)?;
        let count = u16::try_from(row.get::<i64, _>("count")).map_err(StoreError::new)?;
        let limit = u16::try_from(row.get::<i32, _>("warn_limit")).map_err(StoreError::new)?;
        Ok(WarningState {
            count,
            limit,
            reached_limit: count >= limit,
        })
    }

    async fn warning_count(&self, chat_id: ChatId, user_id: UserId) -> Result<u16, StoreError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM warnings WHERE chat_id=$1 AND user_id=$2",
        )
        .bind(chat_id)
        .bind(user_as_i64(user_id)?)
        .fetch_one(&self.pool)
        .await
        .map_err(StoreError::new)?;
        u16::try_from(count).map_err(StoreError::new)
    }

    async fn clear_warnings(&self, chat_id: ChatId, user_id: UserId) -> Result<u64, StoreError> {
        let result = sqlx::query("DELETE FROM warnings WHERE chat_id=$1 AND user_id=$2")
            .bind(chat_id)
            .bind(user_as_i64(user_id)?)
            .execute(&self.pool)
            .await
            .map_err(StoreError::new)?;
        Ok(result.rows_affected())
    }

    async fn audit(&self, event: &AuditEvent) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO moderation_audit(chat_id,actor_id,target_id,action,reason,created_at) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(event.chat_id).bind(event.actor_id.map(user_as_i64).transpose()?)
            .bind(user_as_i64(event.target_id)?).bind(&event.action).bind(&event.reason).bind(event.created_at)
            .execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use chrono::Utc;
    use qalqon_core::{AuditEvent, ModerationStore, Sanction};

    use super::{AuditFilter, MemberUpsert, PgModerationStore, RichAuditEvent};
    use serde_json::json;

    fn unique_chat_id() -> i64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        -i64::try_from(nanos % 1_000_000_000_000).expect("bounded timestamp must fit i64")
            - 1_000_000_000_000
    }

    async fn test_store() -> Option<PgModerationStore> {
        let url = match std::env::var("TEST_DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("TEST_DATABASE_URL yo'q: PostgreSQL integration testi skip qilindi");
                return None;
            }
        };
        Some(
            PgModerationStore::connect(&url, 10)
                .await
                .expect("test PostgreSQL must be reachable and migrations must succeed"),
        )
    }

    #[tokio::test]
    async fn postgres_roundtrip_and_constraints() {
        let Some(store) = test_store().await else {
            return;
        };
        let chat_id = unique_chat_id();
        let user_id = 998_001_u64;

        let defaults = store.settings(chat_id).await.expect("default settings");
        assert_eq!(defaults.flood_limit, 8);
        assert_eq!(defaults.warn_limit, 3);

        store
            .set_flood(chat_id, 12, 30, Sanction::Ban)
            .await
            .expect("set flood");
        store
            .set_warn_limit(chat_id, 2)
            .await
            .expect("set warning limit");
        store
            .set_rules(chat_id, "Test qoidalari")
            .await
            .expect("set rules");
        store
            .set_welcome(chat_id, false, Some("Salom {first_name}"))
            .await
            .expect("set welcome");

        let updated = store.settings(chat_id).await.expect("updated settings");
        assert_eq!(updated.flood_limit, 12);
        assert_eq!(updated.flood_window_secs, 30);
        assert_eq!(updated.flood_action, Sanction::Ban);
        assert_eq!(updated.warn_limit, 2);
        assert!(!updated.welcome_enabled);
        assert_eq!(updated.rules, "Test qoidalari");

        assert!(
            store
                .add_blocked_term(chat_id, "Spam")
                .await
                .expect("add term")
        );
        assert!(
            !store
                .add_blocked_term(chat_id, "spam")
                .await
                .expect("case-insensitive duplicate")
        );
        assert_eq!(
            store.blocked_terms(chat_id).await.expect("terms"),
            vec!["Spam"]
        );
        assert!(
            store
                .remove_blocked_term(chat_id, "SPAM")
                .await
                .expect("remove term")
        );

        let first = store
            .add_warning(chat_id, user_id, Some(42), "birinchi")
            .await
            .expect("first warning");
        let second = store
            .add_warning(chat_id, user_id, Some(42), "ikkinchi")
            .await
            .expect("second warning");
        assert_eq!(first.count, 1);
        assert!(!first.reached_limit);
        assert_eq!(second.count, 2);
        assert!(second.reached_limit);
        assert_eq!(
            store
                .warning_count(chat_id, user_id)
                .await
                .expect("warning count"),
            2
        );

        store
            .audit(&AuditEvent {
                chat_id,
                actor_id: Some(42),
                target_id: user_id,
                action: "warn".into(),
                reason: Some("integration test".into()),
                created_at: Utc::now(),
            })
            .await
            .expect("audit insert");

        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM moderation_audit WHERE chat_id = $1 AND target_id = $2",
        )
        .bind(chat_id)
        .bind(i64::try_from(user_id).expect("test id fits"))
        .fetch_one(&store.pool)
        .await
        .expect("audit query");
        assert_eq!(audit_count, 1);

        sqlx::query("DELETE FROM moderation_audit WHERE chat_id = $1")
            .bind(chat_id)
            .execute(&store.pool)
            .await
            .expect("audit cleanup");
        sqlx::query("DELETE FROM chat_settings WHERE chat_id = $1")
            .bind(chat_id)
            .execute(&store.pool)
            .await
            .expect("chat cleanup");
    }

    #[tokio::test]
    async fn concurrent_warnings_are_serialized() {
        let Some(store) = test_store().await else {
            return;
        };
        let store = Arc::new(store);
        let chat_id = unique_chat_id();
        let user_id = 998_002_u64;
        store
            .set_warn_limit(chat_id, 20)
            .await
            .expect("set warning limit");

        let mut tasks = Vec::new();
        for index in 0..10 {
            let store = Arc::clone(&store);
            tasks.push(tokio::spawn(async move {
                store
                    .add_warning(chat_id, user_id, Some(42), &format!("parallel-{index}"))
                    .await
                    .expect("parallel warning")
                    .count
            }));
        }

        let mut counts = Vec::new();
        for task in tasks {
            counts.push(task.await.expect("warning task must not panic"));
        }
        counts.sort_unstable();
        assert_eq!(counts, (1_u16..=10).collect::<Vec<_>>());
        assert_eq!(
            store
                .warning_count(chat_id, user_id)
                .await
                .expect("warning count"),
            10
        );

        sqlx::query("DELETE FROM chat_settings WHERE chat_id = $1")
            .bind(chat_id)
            .execute(&store.pool)
            .await
            .expect("chat cleanup");
    }

    #[tokio::test]
    async fn mini_app_backend_roundtrip() {
        let Some(store) = test_store().await else {
            return;
        };
        let chat_id = unique_chat_id();
        let user_id = 998_003_u64;
        store
            .upsert_member(&MemberUpsert {
                chat_id,
                chat_title: "Mini App Test",
                chat_username: Some("mini_app_test"),
                chat_type: "supergroup",
                user_id,
                username: Some("Alisher"),
                first_name: "Alisher",
                last_name: Some("Test"),
                is_bot: false,
                is_admin: Some(true),
                status: "administrator",
            })
            .await
            .expect("index member");
        assert_eq!(
            store
                .members(chat_id, Some("alish"), 10)
                .await
                .expect("search member")
                .len(),
            1
        );

        let entry = store
            .add_blocklist_entry(chat_id, "Reklama")
            .await
            .expect("add blocklist")
            .expect("new entry");
        store
            .record_blocklist_match(chat_id, "reklama")
            .await
            .expect("record match");
        let entries = store
            .blocklist(chat_id, Some("rek"), 10)
            .await
            .expect("list blocklist");
        assert_eq!(entries[0].id, entry.id);
        assert_eq!(entries[0].match_count, 1);

        store
            .rich_audit(&RichAuditEvent {
                chat_id,
                actor_id: Some(42),
                target_id: user_id,
                action: "mute",
                reason: Some("API test"),
                source: "admin",
                status: "success",
                duration_secs: Some(3600),
                telegram_message_id: Some(11),
                telegram_update_id: Some(12),
                metadata: json!({"origin":"integration"}),
            })
            .await
            .expect("rich audit");
        let audits = store
            .audit_records(
                chat_id,
                &AuditFilter {
                    action: Some("mute".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("read audit");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].duration_secs, Some(3600));

        store
            .update_runtime(chat_id, true, true, true, true, true)
            .await
            .expect("runtime health");
        assert!(
            store
                .runtime_health(chat_id)
                .await
                .expect("read runtime")
                .bot_admin
        );
        assert_eq!(
            store.modules(chat_id).await.expect("module registry").len(),
            15
        );
        store
            .create_incident(chat_id, "anti_flood", "high", json!({"test":true}))
            .await
            .expect("create incident");
        assert_eq!(
            store
                .incident_metrics(chat_id)
                .await
                .expect("incident metrics")
                .open_incidents,
            1
        );

        sqlx::query("DELETE FROM moderation_audit WHERE chat_id = $1")
            .bind(chat_id)
            .execute(&store.pool)
            .await
            .expect("audit cleanup");
        sqlx::query("DELETE FROM chat_settings WHERE chat_id = $1")
            .bind(chat_id)
            .execute(&store.pool)
            .await
            .expect("chat cleanup");
    }
}
