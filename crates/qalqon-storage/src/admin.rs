use chrono::{DateTime, Utc};
use qalqon_core::{ChatSettings, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

use super::{PgModerationStore, user_as_i64};

#[derive(Debug, Clone, Serialize)]
pub struct ManagedChat {
    pub chat_id: i64,
    pub title: String,
    pub username: Option<String>,
    pub chat_type: String,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMemberRecord {
    pub chat_id: i64,
    pub user_id: u64,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub is_bot: bool,
    pub is_admin: bool,
    pub status: String,
    pub last_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemberUpsert<'a> {
    pub chat_id: i64,
    pub chat_title: &'a str,
    pub chat_username: Option<&'a str>,
    pub chat_type: &'a str,
    pub user_id: u64,
    pub username: Option<&'a str>,
    pub first_name: &'a str,
    pub last_name: Option<&'a str>,
    pub is_bot: bool,
    pub is_admin: Option<bool>,
    pub status: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockedTermRecord {
    pub id: i64,
    pub term: String,
    pub match_count: i64,
    pub last_matched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RichAuditEvent<'a> {
    pub chat_id: i64,
    pub actor_id: Option<u64>,
    pub target_id: u64,
    pub action: &'a str,
    pub reason: Option<&'a str>,
    pub source: &'a str,
    pub status: &'a str,
    pub duration_secs: Option<u64>,
    pub telegram_message_id: Option<i64>,
    pub telegram_update_id: Option<i64>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuditFilter {
    pub action: Option<String>,
    pub source: Option<String>,
    pub target_user_id: Option<u64>,
    pub q: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditRecord {
    pub id: i64,
    pub chat_id: i64,
    pub actor_id: Option<u64>,
    pub target_id: u64,
    pub action: String,
    pub reason: Option<String>,
    pub source: String,
    pub status: String,
    pub duration_secs: Option<i64>,
    pub telegram_message_id: Option<i64>,
    pub telegram_update_id: Option<i64>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditStats {
    pub total: i64,
    pub success: i64,
    pub failed: i64,
    pub auto: i64,
    pub admin: i64,
    pub today: i64,
    pub warnings_today: i64,
    pub auto_blocks_today: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtectionModule {
    pub module_key: String,
    pub title: String,
    pub enabled: bool,
    pub healthy: bool,
    pub configured: bool,
    pub last_triggered_at: Option<DateTime<Utc>>,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHealth {
    pub bot_present: bool,
    pub bot_admin: bool,
    pub can_delete_messages: bool,
    pub can_restrict_members: bool,
    pub can_ban_members: bool,
    pub last_update_at: Option<DateTime<Utc>>,
    pub last_permission_check_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentMetrics {
    pub open_incidents: i64,
    pub average_response_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModerationIncident {
    pub id: i64,
    pub chat_id: i64,
    pub incident_type: String,
    pub severity: String,
    pub detected_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<u64>,
    pub status: String,
    pub metadata: Value,
}

impl PgModerationStore {
    pub(crate) async fn ensure_admin_rows(&self, chat_id: i64) -> Result<(), StoreError> {
        self.ensure_chat(chat_id).await?;
        sqlx::query("INSERT INTO managed_chats(chat_id,title,chat_type) VALUES($1,'Telegram guruhi','supergroup') ON CONFLICT DO NOTHING")
            .bind(chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query("INSERT INTO chat_runtime(chat_id) VALUES($1) ON CONFLICT DO NOTHING")
            .bind(chat_id)
            .execute(&self.pool)
            .await
            .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO protection_modules(chat_id,module_key,title,enabled,configured)
            SELECT $1, v.module_key, v.title, v.enabled, v.configured FROM (VALUES
              ('telegram_permissions','Telegram ruxsatlari',TRUE,FALSE),
              ('database','PostgreSQL',TRUE,TRUE),('admin_auth','Admin autentifikatsiyasi',TRUE,TRUE),
              ('anti_flood','Anti-flood',TRUE,TRUE),('warning_policy','Warning siyosati',TRUE,TRUE),
              ('blocklist','Blocklist',TRUE,FALSE),('welcome','Welcome',TRUE,TRUE),
              ('rules','Guruh qoidalari',TRUE,TRUE),('audit','Audit log',TRUE,TRUE),
              ('member_index','Aʼzolar indeksi',TRUE,FALSE),('captcha','CAPTCHA',FALSE,FALSE),
              ('anti_raid','Anti-raid',FALSE,FALSE),('link_filter','Link filtri',FALSE,FALSE),
              ('reports','Shikoyatlar',FALSE,FALSE),('incident_response','Incident response',TRUE,TRUE)
            ) AS v(module_key,title,enabled,configured) ON CONFLICT DO NOTHING"#,
        )
        .bind(chat_id)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn upsert_member(&self, value: &MemberUpsert<'_>) -> Result<(), StoreError> {
        self.ensure_admin_rows(value.chat_id).await?;
        sqlx::query("UPDATE managed_chats SET title=$2,username=$3,chat_type=$4,last_seen_at=NOW(),updated_at=NOW() WHERE chat_id=$1")
            .bind(value.chat_id).bind(value.chat_title).bind(value.chat_username).bind(value.chat_type)
            .execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO chat_members(chat_id,user_id,username,first_name,last_name,is_bot,is_admin,status)
            VALUES($1,$2,$3,$4,$5,$6,COALESCE($7,FALSE),$8)
            ON CONFLICT(chat_id,user_id) DO UPDATE SET username=EXCLUDED.username,
              first_name=EXCLUDED.first_name,last_name=EXCLUDED.last_name,is_bot=EXCLUDED.is_bot,
              is_admin=COALESCE($7,chat_members.is_admin),status=EXCLUDED.status,
              last_seen_at=NOW(),updated_at=NOW()"#,
        )
        .bind(value.chat_id)
        .bind(user_as_i64(value.user_id)?)
        .bind(value.username)
        .bind(value.first_name)
        .bind(value.last_name)
        .bind(value.is_bot)
        .bind(value.is_admin)
        .bind(value.status)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET configured=TRUE,healthy=TRUE,updated_at=NOW() WHERE chat_id=$1 AND module_key='member_index'")
            .bind(value.chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query(
            "UPDATE chat_runtime SET last_update_at=NOW(),updated_at=NOW() WHERE chat_id=$1",
        )
        .bind(value.chat_id)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn migrate_chat(
        &self,
        old_chat_id: i64,
        new_chat_id: i64,
    ) -> Result<bool, StoreError> {
        if old_chat_id == new_chat_id {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await.map_err(StoreError::new)?;
        let old_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_settings WHERE chat_id=$1)")
                .bind(old_chat_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(StoreError::new)?;
        if !old_exists {
            tx.rollback().await.map_err(StoreError::new)?;
            return Ok(false);
        }

        sqlx::query(
            r#"INSERT INTO chat_settings(
                chat_id,flood_limit,flood_window_secs,flood_action,warn_limit,warn_action,
                mute_duration_secs,welcome_enabled,welcome_template,rules,updated_at)
              SELECT $2,flood_limit,flood_window_secs,flood_action,warn_limit,warn_action,
                mute_duration_secs,welcome_enabled,welcome_template,rules,updated_at
              FROM chat_settings WHERE chat_id=$1 ON CONFLICT(chat_id) DO NOTHING"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"UPDATE chat_settings AS target SET
                flood_limit=source.flood_limit,flood_window_secs=source.flood_window_secs,
                flood_action=source.flood_action,warn_limit=source.warn_limit,
                warn_action=source.warn_action,mute_duration_secs=source.mute_duration_secs,
                welcome_enabled=source.welcome_enabled,welcome_template=source.welcome_template,
                rules=source.rules,updated_at=GREATEST(target.updated_at,source.updated_at)
              FROM chat_settings AS source
              WHERE target.chat_id=$2 AND source.chat_id=$1"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO managed_chats(chat_id,title,username,chat_type,last_seen_at,updated_at)
              SELECT $2,title,username,'supergroup',last_seen_at,updated_at
              FROM managed_chats WHERE chat_id=$1
              ON CONFLICT(chat_id) DO UPDATE SET
                title=CASE WHEN managed_chats.title='Telegram guruhi' THEN EXCLUDED.title ELSE managed_chats.title END,
                username=COALESCE(managed_chats.username,EXCLUDED.username),chat_type='supergroup',
                last_seen_at=GREATEST(managed_chats.last_seen_at,EXCLUDED.last_seen_at),
                updated_at=GREATEST(managed_chats.updated_at,EXCLUDED.updated_at)"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO chat_members(
                chat_id,user_id,username,first_name,last_name,is_bot,is_admin,status,last_seen_at,updated_at)
              SELECT $2,user_id,username,first_name,last_name,is_bot,is_admin,status,last_seen_at,updated_at
              FROM chat_members WHERE chat_id=$1
              ON CONFLICT(chat_id,user_id) DO UPDATE SET
                username=COALESCE(chat_members.username,EXCLUDED.username),
                first_name=chat_members.first_name,last_name=COALESCE(chat_members.last_name,EXCLUDED.last_name),
                is_bot=chat_members.is_bot OR EXCLUDED.is_bot,
                is_admin=chat_members.is_admin OR EXCLUDED.is_admin,
                last_seen_at=GREATEST(chat_members.last_seen_at,EXCLUDED.last_seen_at),
                updated_at=GREATEST(chat_members.updated_at,EXCLUDED.updated_at)"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO blocked_terms(chat_id,term,created_at,match_count,last_matched_at,updated_at)
              SELECT $2,term,created_at,match_count,last_matched_at,updated_at
              FROM blocked_terms WHERE chat_id=$1 ON CONFLICT DO NOTHING"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO chat_runtime(
                chat_id,bot_present,bot_admin,can_delete_messages,can_restrict_members,
                can_ban_members,last_update_at,last_permission_check_at,updated_at)
              SELECT $2,bot_present,bot_admin,can_delete_messages,can_restrict_members,
                can_ban_members,last_update_at,last_permission_check_at,updated_at
              FROM chat_runtime WHERE chat_id=$1
              ON CONFLICT(chat_id) DO UPDATE SET
                bot_present=chat_runtime.bot_present OR EXCLUDED.bot_present,
                bot_admin=chat_runtime.bot_admin OR EXCLUDED.bot_admin,
                can_delete_messages=chat_runtime.can_delete_messages OR EXCLUDED.can_delete_messages,
                can_restrict_members=chat_runtime.can_restrict_members OR EXCLUDED.can_restrict_members,
                can_ban_members=chat_runtime.can_ban_members OR EXCLUDED.can_ban_members,
                last_update_at=GREATEST(chat_runtime.last_update_at,EXCLUDED.last_update_at),
                last_permission_check_at=GREATEST(chat_runtime.last_permission_check_at,EXCLUDED.last_permission_check_at),
                updated_at=GREATEST(chat_runtime.updated_at,EXCLUDED.updated_at)"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"INSERT INTO protection_modules(
                chat_id,module_key,title,enabled,healthy,configured,last_triggered_at,config,updated_at)
              SELECT $2,module_key,title,enabled,healthy,configured,last_triggered_at,config,updated_at
              FROM protection_modules WHERE chat_id=$1
              ON CONFLICT(chat_id,module_key) DO UPDATE SET
                title=EXCLUDED.title,enabled=EXCLUDED.enabled,healthy=EXCLUDED.healthy,
                configured=EXCLUDED.configured,last_triggered_at=GREATEST(
                  protection_modules.last_triggered_at,EXCLUDED.last_triggered_at),
                config=EXCLUDED.config,updated_at=GREATEST(protection_modules.updated_at,EXCLUDED.updated_at)"#,
        )
        .bind(old_chat_id)
        .bind(new_chat_id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::new)?;
        for table in ["warnings", "moderation_incidents", "moderation_audit"] {
            let query = format!("UPDATE {table} SET chat_id=$2 WHERE chat_id=$1");
            sqlx::query(&query)
                .bind(old_chat_id)
                .bind(new_chat_id)
                .execute(&mut *tx)
                .await
                .map_err(StoreError::new)?;
        }
        sqlx::query("DELETE FROM chat_settings WHERE chat_id=$1")
            .bind(old_chat_id)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::new)?;
        tx.commit().await.map_err(StoreError::new)?;
        Ok(true)
    }

    pub async fn update_member_status(
        &self,
        chat_id: i64,
        user_id: u64,
        status: &str,
    ) -> Result<bool, StoreError> {
        if !matches!(
            status,
            "creator" | "administrator" | "member" | "restricted" | "left" | "kicked"
        ) {
            return Err(StoreError::new("invalid chat member status"));
        }
        let result = sqlx::query(
            "UPDATE chat_members SET status=$3,updated_at=NOW() WHERE chat_id=$1 AND user_id=$2",
        )
        .bind(chat_id)
        .bind(user_as_i64(user_id)?)
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_chats(&self) -> Result<Vec<ManagedChat>, StoreError> {
        let rows = sqlx::query("SELECT chat_id,title,username,chat_type,last_seen_at FROM managed_chats ORDER BY last_seen_at DESC")
            .fetch_all(&self.pool).await.map_err(StoreError::new)?;
        Ok(rows.into_iter().map(map_chat).collect())
    }

    pub async fn managed_chat(&self, chat_id: i64) -> Result<Option<ManagedChat>, StoreError> {
        let row = sqlx::query("SELECT chat_id,title,username,chat_type,last_seen_at FROM managed_chats WHERE chat_id=$1")
            .bind(chat_id).fetch_optional(&self.pool).await.map_err(StoreError::new)?;
        Ok(row.map(map_chat))
    }

    pub async fn members(
        &self,
        chat_id: i64,
        q: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ChatMemberRecord>, StoreError> {
        let terms = q.and_then(|value| {
            let terms = value
                .split_whitespace()
                .map(|term| term.trim_start_matches('@').to_lowercase())
                .filter(|term| !term.is_empty())
                .map(|term| format!("%{term}%"))
                .collect::<Vec<_>>();
            (!terms.is_empty()).then_some(terms)
        });
        let rows = sqlx::query(
            r#"SELECT chat_id,user_id,username,first_name,last_name,is_bot,is_admin,status,last_seen_at,updated_at
            FROM chat_members WHERE chat_id=$1 AND ($2::TEXT[] IS NULL OR NOT EXISTS (
              SELECT 1 FROM unnest($2::TEXT[]) AS search_term(value)
              WHERE lower(concat_ws(' ',first_name,last_name,username,user_id::TEXT)) NOT LIKE search_term.value
            ))
            ORDER BY last_seen_at DESC LIMIT $3"#,
        ).bind(chat_id).bind(terms).bind(limit.clamp(1, 100))
          .fetch_all(&self.pool).await.map_err(StoreError::new)?;
        rows.into_iter().map(map_member).collect()
    }

    pub async fn member(
        &self,
        chat_id: i64,
        user_id: u64,
    ) -> Result<Option<ChatMemberRecord>, StoreError> {
        let row = sqlx::query("SELECT chat_id,user_id,username,first_name,last_name,is_bot,is_admin,status,last_seen_at,updated_at FROM chat_members WHERE chat_id=$1 AND user_id=$2")
            .bind(chat_id).bind(user_as_i64(user_id)?).fetch_optional(&self.pool).await.map_err(StoreError::new)?;
        row.map(map_member).transpose()
    }

    pub async fn replace_settings(&self, value: &ChatSettings) -> Result<(), StoreError> {
        self.ensure_admin_rows(value.chat_id).await?;
        sqlx::query(
            r#"UPDATE chat_settings SET flood_limit=$2,flood_window_secs=$3,flood_action=$4,
            warn_limit=$5,warn_action=$6,mute_duration_secs=$7,welcome_enabled=$8,
            welcome_template=$9,rules=$10,updated_at=NOW() WHERE chat_id=$1"#,
        )
        .bind(value.chat_id)
        .bind(i32::from(value.flood_limit))
        .bind(i32::try_from(value.flood_window_secs).map_err(StoreError::new)?)
        .bind(value.flood_action.as_str())
        .bind(i32::from(value.warn_limit))
        .bind(value.warn_action.as_str())
        .bind(i32::try_from(value.mute_duration_secs).map_err(StoreError::new)?)
        .bind(value.welcome_enabled)
        .bind(&value.welcome_template)
        .bind(&value.rules)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        sqlx::query(
            r#"UPDATE protection_modules SET
              enabled=CASE module_key
                WHEN 'anti_flood' THEN $2 > 0
                WHEN 'welcome' THEN $8
                WHEN 'rules' THEN $10 <> ''
                ELSE enabled END,
              configured=CASE module_key
                WHEN 'anti_flood' THEN TRUE
                WHEN 'warning_policy' THEN TRUE
                WHEN 'welcome' THEN $9 <> ''
                WHEN 'rules' THEN $10 <> ''
                ELSE configured END,
              config=CASE module_key
                WHEN 'anti_flood' THEN jsonb_build_object('limit',$2,'window_secs',$3,'action',$4)
                WHEN 'warning_policy' THEN jsonb_build_object('limit',$5,'action',$6,'mute_duration_secs',$7)
                WHEN 'welcome' THEN jsonb_build_object('template',$9)
                WHEN 'rules' THEN jsonb_build_object('rules',$10)
                ELSE config END,
              updated_at=NOW()
            WHERE chat_id=$1 AND module_key IN ('anti_flood','warning_policy','welcome','rules')"#,
        )
        .bind(value.chat_id)
        .bind(i32::from(value.flood_limit))
        .bind(i32::try_from(value.flood_window_secs).map_err(StoreError::new)?)
        .bind(value.flood_action.as_str())
        .bind(i32::from(value.warn_limit))
        .bind(value.warn_action.as_str())
        .bind(i32::try_from(value.mute_duration_secs).map_err(StoreError::new)?)
        .bind(value.welcome_enabled)
        .bind(&value.welcome_template)
        .bind(value.rules.trim())
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn blocklist(
        &self,
        chat_id: i64,
        q: Option<&str>,
        limit: i64,
    ) -> Result<Vec<BlockedTermRecord>, StoreError> {
        let pattern = q.map(|value| format!("%{}%", value.trim().to_lowercase()));
        let rows = sqlx::query("SELECT id,term,match_count,last_matched_at,created_at,updated_at FROM blocked_terms WHERE chat_id=$1 AND ($2::TEXT IS NULL OR lower(term) LIKE $2) ORDER BY updated_at DESC,id DESC LIMIT $3")
            .bind(chat_id).bind(pattern).bind(limit.clamp(1,200)).fetch_all(&self.pool).await.map_err(StoreError::new)?;
        Ok(rows.into_iter().map(map_blocked_term).collect())
    }

    pub async fn add_blocklist_entry(
        &self,
        chat_id: i64,
        term: &str,
    ) -> Result<Option<BlockedTermRecord>, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        let row = sqlx::query("INSERT INTO blocked_terms(chat_id,term) VALUES($1,$2) ON CONFLICT DO NOTHING RETURNING id,term,match_count,last_matched_at,created_at,updated_at")
            .bind(chat_id).bind(term.trim()).fetch_optional(&self.pool).await.map_err(StoreError::new)?;
        if row.is_some() {
            sqlx::query("UPDATE protection_modules SET configured=TRUE,enabled=TRUE,updated_at=NOW() WHERE chat_id=$1 AND module_key='blocklist'")
                .bind(chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        }
        Ok(row.map(map_blocked_term))
    }

    pub async fn delete_blocklist_entry(
        &self,
        chat_id: i64,
        term_id: i64,
    ) -> Result<Option<String>, StoreError> {
        let term = sqlx::query_scalar::<_, String>(
            "DELETE FROM blocked_terms WHERE chat_id=$1 AND id=$2 RETURNING term",
        )
        .bind(chat_id)
        .bind(term_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::new)?;
        if term.is_some() {
            sqlx::query("UPDATE protection_modules SET configured=EXISTS(SELECT 1 FROM blocked_terms WHERE chat_id=$1),updated_at=NOW() WHERE chat_id=$1 AND module_key='blocklist'")
                .bind(chat_id).execute(&self.pool).await.map_err(StoreError::new)?;
        }
        Ok(term)
    }

    pub async fn record_blocklist_match(&self, chat_id: i64, term: &str) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(StoreError::new)?;
        sqlx::query("UPDATE blocked_terms SET match_count=match_count+1,last_matched_at=NOW(),updated_at=NOW() WHERE chat_id=$1 AND lower(term)=lower($2)")
            .bind(chat_id).bind(term).execute(&mut *tx).await.map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET last_triggered_at=NOW(),healthy=TRUE,updated_at=NOW() WHERE chat_id=$1 AND module_key='blocklist'")
            .bind(chat_id).execute(&mut *tx).await.map_err(StoreError::new)?;
        tx.commit().await.map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn rich_audit(&self, event: &RichAuditEvent<'_>) -> Result<(), StoreError> {
        self.ensure_admin_rows(event.chat_id).await?;
        sqlx::query(
            r#"INSERT INTO moderation_audit(chat_id,actor_id,target_id,action,reason,created_at,
            source,status,duration_secs,telegram_message_id,telegram_update_id,metadata)
            VALUES($1,$2,$3,$4,$5,NOW(),$6,$7,$8,$9,$10,$11)"#,
        )
        .bind(event.chat_id)
        .bind(event.actor_id.map(user_as_i64).transpose()?)
        .bind(user_as_i64(event.target_id)?)
        .bind(event.action)
        .bind(event.reason)
        .bind(event.source)
        .bind(event.status)
        .bind(
            event
                .duration_secs
                .map(i64::try_from)
                .transpose()
                .map_err(StoreError::new)?,
        )
        .bind(event.telegram_message_id)
        .bind(event.telegram_update_id)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await
        .map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn audit_records(
        &self,
        chat_id: i64,
        filter: &AuditFilter,
    ) -> Result<Vec<AuditRecord>, StoreError> {
        let q = filter
            .q
            .as_ref()
            .map(|value| format!("%{}%", value.trim().to_lowercase()));
        let rows = sqlx::query(
            r#"SELECT a.id,a.chat_id,a.actor_id,a.target_id,a.action,a.reason,a.source,a.status,
              a.duration_secs,a.telegram_message_id,a.telegram_update_id,a.metadata,a.created_at
            FROM moderation_audit a WHERE a.chat_id=$1
              AND ($2::TEXT IS NULL OR a.action=$2) AND ($3::TEXT IS NULL OR a.source=$3)
              AND ($4::BIGINT IS NULL OR a.target_id=$4)
              AND ($5::TIMESTAMPTZ IS NULL OR a.created_at >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR a.created_at <= $6)
              AND ($7::BIGINT IS NULL OR a.id < $7)
              AND ($8::TEXT IS NULL OR lower(COALESCE(a.reason,'')) LIKE $8
                OR a.actor_id::TEXT LIKE $8 OR a.target_id::TEXT LIKE $8
                OR EXISTS(SELECT 1 FROM chat_members m WHERE m.chat_id=a.chat_id
                  AND m.user_id IN (a.actor_id,a.target_id) AND
                  (lower(COALESCE(m.username,'')) LIKE $8 OR lower(m.first_name) LIKE $8 OR lower(COALESCE(m.last_name,'')) LIKE $8)))
            ORDER BY a.id DESC LIMIT $9"#,
        ).bind(chat_id).bind(&filter.action).bind(&filter.source)
          .bind(filter.target_user_id.map(user_as_i64).transpose()?)
          .bind(filter.from).bind(filter.to).bind(filter.cursor).bind(q)
          .bind(filter.limit.unwrap_or(50).clamp(1,10_000))
          .fetch_all(&self.pool).await.map_err(StoreError::new)?;
        rows.into_iter().map(map_audit).collect()
    }

    pub async fn audit_stats(&self, chat_id: i64) -> Result<AuditStats, StoreError> {
        let row = sqlx::query(
            r#"SELECT COUNT(*)::BIGINT total,COUNT(*) FILTER(WHERE status='success')::BIGINT success,
              COUNT(*) FILTER(WHERE status='failed')::BIGINT failed,COUNT(*) FILTER(WHERE source='auto')::BIGINT auto,
              COUNT(*) FILTER(WHERE source='admin')::BIGINT admin,
              COUNT(*) FILTER(WHERE created_at >= date_trunc('day',NOW()))::BIGINT today,
              COUNT(*) FILTER(WHERE action='warn' AND created_at >= date_trunc('day',NOW()))::BIGINT warnings_today,
              COUNT(*) FILTER(WHERE source='auto' AND status='success' AND action IN ('delete','mute','ban','blocklist_match','anti_flood') AND created_at >= date_trunc('day',NOW()))::BIGINT auto_blocks_today
            FROM moderation_audit WHERE chat_id=$1"#,
        ).bind(chat_id).fetch_one(&self.pool).await.map_err(StoreError::new)?;
        Ok(AuditStats {
            total: row.get("total"),
            success: row.get("success"),
            failed: row.get("failed"),
            auto: row.get("auto"),
            admin: row.get("admin"),
            today: row.get("today"),
            warnings_today: row.get("warnings_today"),
            auto_blocks_today: row.get("auto_blocks_today"),
        })
    }

    pub async fn modules(&self, chat_id: i64) -> Result<Vec<ProtectionModule>, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        let rows = sqlx::query("SELECT module_key,title,enabled,healthy,configured,last_triggered_at,config,updated_at FROM protection_modules WHERE chat_id=$1 ORDER BY module_key")
            .bind(chat_id).fetch_all(&self.pool).await.map_err(StoreError::new)?;
        Ok(rows.into_iter().map(map_module).collect())
    }

    pub async fn update_module(
        &self,
        chat_id: i64,
        key: &str,
        enabled: Option<bool>,
        config: Option<Value>,
    ) -> Result<Option<ProtectionModule>, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        let row = sqlx::query("UPDATE protection_modules SET enabled=COALESCE($3,enabled),config=COALESCE($4,config),configured=CASE WHEN $4::JSONB IS NULL THEN configured ELSE TRUE END,updated_at=NOW() WHERE chat_id=$1 AND module_key=$2 RETURNING module_key,title,enabled,healthy,configured,last_triggered_at,config,updated_at")
            .bind(chat_id).bind(key).bind(enabled).bind(config).fetch_optional(&self.pool).await.map_err(StoreError::new)?;
        Ok(row.map(map_module))
    }

    pub async fn module_enabled(&self, chat_id: i64, key: &str) -> Result<bool, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query_scalar(
            "SELECT enabled FROM protection_modules WHERE chat_id=$1 AND module_key=$2",
        )
        .bind(chat_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::new)
        .map(|value| value.unwrap_or(false))
    }

    pub async fn update_runtime(
        &self,
        chat_id: i64,
        bot_present: bool,
        bot_admin: bool,
        can_delete: bool,
        can_restrict: bool,
        can_ban: bool,
    ) -> Result<(), StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query("UPDATE chat_runtime SET bot_present=$2,bot_admin=$3,can_delete_messages=$4,can_restrict_members=$5,can_ban_members=$6,last_permission_check_at=NOW(),updated_at=NOW() WHERE chat_id=$1")
            .bind(chat_id).bind(bot_present).bind(bot_admin).bind(can_delete).bind(can_restrict).bind(can_ban)
            .execute(&self.pool).await.map_err(StoreError::new)?;
        sqlx::query("UPDATE protection_modules SET healthy=$2,configured=$2,updated_at=NOW() WHERE chat_id=$1 AND module_key='telegram_permissions'")
            .bind(chat_id).bind(bot_admin && can_delete && can_restrict && can_ban)
            .execute(&self.pool).await.map_err(StoreError::new)?;
        Ok(())
    }

    pub async fn runtime_health(&self, chat_id: i64) -> Result<RuntimeHealth, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        let row = sqlx::query("SELECT bot_present,bot_admin,can_delete_messages,can_restrict_members,can_ban_members,last_update_at,last_permission_check_at,updated_at FROM chat_runtime WHERE chat_id=$1")
            .bind(chat_id).fetch_one(&self.pool).await.map_err(StoreError::new)?;
        Ok(RuntimeHealth {
            bot_present: row.get("bot_present"),
            bot_admin: row.get("bot_admin"),
            can_delete_messages: row.get("can_delete_messages"),
            can_restrict_members: row.get("can_restrict_members"),
            can_ban_members: row.get("can_ban_members"),
            last_update_at: row.get("last_update_at"),
            last_permission_check_at: row.get("last_permission_check_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn incident_metrics(&self, chat_id: i64) -> Result<IncidentMetrics, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) FILTER(WHERE status<>'resolved')::BIGINT open_incidents,AVG(EXTRACT(EPOCH FROM (acknowledged_at-detected_at))) FILTER(WHERE acknowledged_at IS NOT NULL)::FLOAT8 average_response_seconds FROM moderation_incidents WHERE chat_id=$1")
            .bind(chat_id).fetch_one(&self.pool).await.map_err(StoreError::new)?;
        Ok(IncidentMetrics {
            open_incidents: row.get("open_incidents"),
            average_response_seconds: row.get("average_response_seconds"),
        })
    }

    pub async fn create_incident(
        &self,
        chat_id: i64,
        incident_type: &str,
        severity: &str,
        metadata: Value,
    ) -> Result<i64, StoreError> {
        self.ensure_admin_rows(chat_id).await?;
        sqlx::query_scalar("INSERT INTO moderation_incidents(chat_id,incident_type,severity,metadata) VALUES($1,$2,$3,$4) RETURNING id")
            .bind(chat_id).bind(incident_type).bind(severity).bind(metadata)
            .fetch_one(&self.pool).await.map_err(StoreError::new)
    }

    pub async fn incidents(
        &self,
        chat_id: i64,
        status: Option<&str>,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ModerationIncident>, StoreError> {
        let rows = sqlx::query(
            r#"SELECT id,chat_id,incident_type,severity,detected_at,acknowledged_at,
              resolved_at,resolved_by,status,metadata FROM moderation_incidents
            WHERE chat_id=$1 AND ($2::TEXT IS NULL OR status=$2)
              AND ($3::BIGINT IS NULL OR id < $3)
            ORDER BY id DESC LIMIT $4"#,
        )
        .bind(chat_id)
        .bind(status)
        .bind(cursor)
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::new)?;
        rows.into_iter().map(map_incident).collect()
    }

    pub async fn update_incident_status(
        &self,
        chat_id: i64,
        incident_id: i64,
        status: &str,
        actor_id: u64,
    ) -> Result<Option<ModerationIncident>, StoreError> {
        let row = sqlx::query(
            r#"UPDATE moderation_incidents SET
              status=$3,
              acknowledged_at=CASE WHEN $3 IN ('acknowledged','resolved')
                THEN COALESCE(acknowledged_at,NOW()) ELSE acknowledged_at END,
              resolved_at=CASE WHEN $3='resolved' THEN COALESCE(resolved_at,NOW()) ELSE NULL END,
              resolved_by=CASE WHEN $3='resolved' THEN $4 ELSE NULL END
            WHERE chat_id=$1 AND id=$2
            RETURNING id,chat_id,incident_type,severity,detected_at,acknowledged_at,
              resolved_at,resolved_by,status,metadata"#,
        )
        .bind(chat_id)
        .bind(incident_id)
        .bind(status)
        .bind(user_as_i64(actor_id)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::new)?;
        row.map(map_incident).transpose()
    }
}

fn map_chat(row: sqlx::postgres::PgRow) -> ManagedChat {
    ManagedChat {
        chat_id: row.get("chat_id"),
        title: row.get("title"),
        username: row.get("username"),
        chat_type: row.get("chat_type"),
        last_seen_at: row.get("last_seen_at"),
    }
}

fn map_member(row: sqlx::postgres::PgRow) -> Result<ChatMemberRecord, StoreError> {
    Ok(ChatMemberRecord {
        chat_id: row.get("chat_id"),
        user_id: u64::try_from(row.get::<i64, _>("user_id")).map_err(StoreError::new)?,
        username: row.get("username"),
        first_name: row.get("first_name"),
        last_name: row.get("last_name"),
        is_bot: row.get("is_bot"),
        is_admin: row.get("is_admin"),
        status: row.get("status"),
        last_seen_at: row.get("last_seen_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_blocked_term(row: sqlx::postgres::PgRow) -> BlockedTermRecord {
    BlockedTermRecord {
        id: row.get("id"),
        term: row.get("term"),
        match_count: row.get("match_count"),
        last_matched_at: row.get("last_matched_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_audit(row: sqlx::postgres::PgRow) -> Result<AuditRecord, StoreError> {
    Ok(AuditRecord {
        id: row.get("id"),
        chat_id: row.get("chat_id"),
        actor_id: row
            .get::<Option<i64>, _>("actor_id")
            .map(u64::try_from)
            .transpose()
            .map_err(StoreError::new)?,
        target_id: u64::try_from(row.get::<i64, _>("target_id")).map_err(StoreError::new)?,
        action: row.get("action"),
        reason: row.get("reason"),
        source: row.get("source"),
        status: row.get("status"),
        duration_secs: row.get("duration_secs"),
        telegram_message_id: row.get("telegram_message_id"),
        telegram_update_id: row.get("telegram_update_id"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

fn map_module(row: sqlx::postgres::PgRow) -> ProtectionModule {
    ProtectionModule {
        module_key: row.get("module_key"),
        title: row.get("title"),
        enabled: row.get("enabled"),
        healthy: row.get("healthy"),
        configured: row.get("configured"),
        last_triggered_at: row.get("last_triggered_at"),
        config: row.get("config"),
        updated_at: row.get("updated_at"),
    }
}

fn map_incident(row: sqlx::postgres::PgRow) -> Result<ModerationIncident, StoreError> {
    Ok(ModerationIncident {
        id: row.get("id"),
        chat_id: row.get("chat_id"),
        incident_type: row.get("incident_type"),
        severity: row.get("severity"),
        detected_at: row.get("detected_at"),
        acknowledged_at: row.get("acknowledged_at"),
        resolved_at: row.get("resolved_at"),
        resolved_by: row
            .get::<Option<i64>, _>("resolved_by")
            .map(u64::try_from)
            .transpose()
            .map_err(StoreError::new)?,
        status: row.get("status"),
        metadata: row.get("metadata"),
    })
}
