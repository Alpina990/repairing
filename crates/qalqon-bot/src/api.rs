use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use qalqon_core::Sanction;
use qalqon_storage::{
    AuditFilter, ManagedChat, MemberUpsert, PgModerationStore, ProtectionModule, RichAuditEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use teloxide::{
    Bot, RequestError,
    payloads::UnbanChatMemberSetters,
    prelude::Requester,
    types::{ChatFullInfo, ChatId, ChatMember, ChatMemberKind, ChatPermissions, Me, UserId},
};
use tokio::net::TcpListener;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnResponse, TraceLayer},
};

use crate::{commands, state::AppState};

type HmacSha256 = Hmac<Sha256>;
type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone)]
pub struct ApiState {
    pub bot: Bot,
    pub me: Me,
    pub app: AppState,
    pub store: PgModerationStore,
    token: Arc<str>,
    auth_max_age: Duration,
}

impl ApiState {
    pub fn new(
        bot: Bot,
        me: Me,
        app: AppState,
        store: PgModerationStore,
        token: String,
        auth_max_age: Duration,
    ) -> Self {
        Self {
            bot,
            me,
            app,
            store,
            token: token.into(),
            auth_max_age,
        }
    }
}

pub async fn bind(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "Mini App API ishga tushdi");
    Ok(listener)
}

pub async fn serve(
    listener: TcpListener,
    state: ApiState,
    allowed_origin: Option<&str>,
) -> anyhow::Result<()> {
    let mut router = Router::new()
        .route("/api/me", get(get_me))
        .route("/api/chats", get(get_chats))
        .route("/api/chats/{chat_id}", get(get_chat))
        .route("/api/chats/{chat_id}/members", get(get_members))
        .route("/api/chats/{chat_id}/members/{user_id}", get(get_member))
        .route(
            "/api/chats/{chat_id}/members/{user_id}/warnings",
            get(get_warnings),
        )
        .route(
            "/api/chats/{chat_id}/moderation/warn",
            post(moderation_warn),
        )
        .route(
            "/api/chats/{chat_id}/moderation/unwarn",
            post(moderation_unwarn),
        )
        .route(
            "/api/chats/{chat_id}/moderation/mute",
            post(moderation_mute),
        )
        .route(
            "/api/chats/{chat_id}/moderation/unmute",
            post(moderation_unmute),
        )
        .route("/api/chats/{chat_id}/moderation/ban", post(moderation_ban))
        .route(
            "/api/chats/{chat_id}/moderation/unban",
            post(moderation_unban),
        )
        .route(
            "/api/chats/{chat_id}/settings",
            get(get_settings).patch(patch_settings),
        )
        .route(
            "/api/chats/{chat_id}/blocklist",
            get(get_blocklist).post(add_blocklist),
        )
        .route(
            "/api/chats/{chat_id}/blocklist/{term_id}",
            delete(delete_blocklist),
        )
        .route("/api/chats/{chat_id}/audit", get(get_audit))
        .route("/api/chats/{chat_id}/audit/stats", get(get_audit_stats))
        .route("/api/chats/{chat_id}/audit/export", get(export_audit))
        .route("/api/chats/{chat_id}/overview", get(get_overview))
        .route("/api/chats/{chat_id}/incidents", get(get_incidents))
        .route(
            "/api/chats/{chat_id}/incidents/{incident_id}",
            patch(patch_incident),
        )
        .route("/api/chats/{chat_id}/modules", get(get_modules))
        .route(
            "/api/chats/{chat_id}/modules/{module_key}",
            patch(patch_module),
        )
        .route("/api/chats/{chat_id}/health", get(get_health))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<Body>| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("unknown");
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id
                    )
                })
                .on_response(DefaultOnResponse::new().include_headers(false)),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state);

    if let Some(origin) = allowed_origin {
        let origin = HeaderValue::from_str(origin)?;
        router = router.layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
                .expose_headers([header::HeaderName::from_static("x-request-id")]),
        );
    }
    axum::serve(listener, router).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MiniAppUser {
    id: u64,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
    language_code: Option<String>,
    #[serde(default)]
    is_premium: bool,
    #[serde(default)]
    allows_write_to_pm: bool,
    photo_url: Option<String>,
}

#[derive(Debug, Clone)]
struct AuthenticatedUser {
    user: MiniAppUser,
    auth_date: i64,
}

fn authenticate(state: &ApiState, headers: &HeaderMap) -> ApiResult<AuthenticatedUser> {
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix("tma ")
                .or_else(|| value.strip_prefix("Bearer "))
        })
        .or_else(|| {
            headers
                .get("x-telegram-init-data")
                .and_then(|value| value.to_str().ok())
        })
        .ok_or_else(|| ApiError::unauthorized("init_data_required", "Telegram initData kerak"))?;

    validate_init_data(raw, &state.token, state.auth_max_age)
}

fn validate_init_data(
    raw: &str,
    token: &str,
    auth_max_age: Duration,
) -> ApiResult<AuthenticatedUser> {
    let mut values = BTreeMap::<String, String>::new();
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        values.insert(key.into_owned(), value.into_owned());
    }
    let hash = values
        .remove("hash")
        .ok_or_else(|| ApiError::unauthorized("invalid_init_data", "initData hash yo'q"))?;
    let provided = hex::decode(hash)
        .map_err(|_| ApiError::unauthorized("invalid_init_data", "initData hash noto'g'ri"))?;
    let check_string = values
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut secret_mac =
        HmacSha256::new_from_slice(b"WebAppData").expect("HMAC accepts keys of any length");
    secret_mac.update(token.as_bytes());
    let secret = secret_mac.finalize().into_bytes();
    let mut data_mac =
        HmacSha256::new_from_slice(&secret).expect("HMAC accepts keys of any length");
    data_mac.update(check_string.as_bytes());
    data_mac
        .verify_slice(&provided)
        .map_err(|_| ApiError::unauthorized("invalid_init_data", "initData imzosi noto'g'ri"))?;

    let auth_date = values
        .get("auth_date")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| ApiError::unauthorized("invalid_init_data", "auth_date noto'g'ri"))?;
    let now = Utc::now().timestamp();
    let max_age = i64::try_from(auth_max_age.as_secs()).unwrap_or(i64::MAX);
    if auth_date > now + 30 || now.saturating_sub(auth_date) > max_age {
        return Err(ApiError::unauthorized(
            "init_data_expired",
            "initData muddati tugagan",
        ));
    }
    let user = values
        .get("user")
        .ok_or_else(|| ApiError::unauthorized("invalid_init_data", "user yo'q"))?;
    let user = serde_json::from_str::<MiniAppUser>(user)
        .map_err(|_| ApiError::unauthorized("invalid_init_data", "user JSON noto'g'ri"))?;
    Ok(AuthenticatedUser { user, auth_date })
}

async fn require_admin(
    state: &ApiState,
    headers: &HeaderMap,
    chat_id: i64,
) -> ApiResult<AuthenticatedUser> {
    let auth = authenticate(state, headers)?;
    let chat = state
        .bot
        .get_chat(ChatId(chat_id))
        .await
        .map_err(ApiError::telegram)?;
    if !chat.is_group() && !chat.is_supergroup() {
        return Err(ApiError::bad_request(
            "group_only",
            "Endpoint faqat guruh uchun",
        ));
    }
    let member = state
        .bot
        .get_chat_member(ChatId(chat_id), UserId(auth.user.id))
        .await
        .map_err(ApiError::telegram)?;
    if !member.is_privileged() {
        return Err(ApiError::forbidden(
            "admin_required",
            "Guruh administratori bo'lish kerak",
        ));
    }
    index_telegram_member(&state.store, chat_id, &chat, &member).await?;
    Ok(auth)
}

async fn index_telegram_member(
    store: &PgModerationStore,
    chat_id: i64,
    chat: &ChatFullInfo,
    member: &ChatMember,
) -> ApiResult<()> {
    store
        .upsert_member(&MemberUpsert {
            chat_id,
            chat_title: chat.title().unwrap_or("Telegram guruhi"),
            chat_username: chat.username(),
            chat_type: if chat.is_supergroup() {
                "supergroup"
            } else {
                "group"
            },
            user_id: member.user.id.0,
            username: member.user.username.as_deref(),
            first_name: &member.user.first_name,
            last_name: member.user.last_name.as_deref(),
            is_bot: member.user.is_bot,
            is_admin: Some(member.is_privileged()),
            status: telegram_member_status(&member.kind),
        })
        .await
        .map_err(ApiError::store)
}

async fn sync_chat_administrators(state: &ApiState, chat_id: i64) -> ApiResult<()> {
    let chat = state
        .bot
        .get_chat(ChatId(chat_id))
        .await
        .map_err(ApiError::telegram)?;
    let administrators = state
        .bot
        .get_chat_administrators(ChatId(chat_id))
        .await
        .map_err(ApiError::telegram)?;
    for member in administrators {
        index_telegram_member(&state.store, chat_id, &chat, &member).await?;
    }
    Ok(())
}

async fn get_me(State(state): State<ApiState>, headers: HeaderMap) -> ApiResult<impl IntoResponse> {
    let auth = authenticate(&state, &headers)?;
    Ok(ok(
        json!({ "user": auth.user, "auth_date": auth.auth_date }),
    ))
}

async fn get_chats(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    let auth = authenticate(&state, &headers)?;
    let mut allowed: Vec<ManagedChat> = Vec::new();
    for listed_chat in state.store.list_chats().await.map_err(ApiError::store)? {
        let (chat_id, member) = match state
            .bot
            .get_chat_member(ChatId(listed_chat.chat_id), UserId(auth.user.id))
            .await
        {
            Ok(member) => (listed_chat.chat_id, member),
            Err(error) => {
                let Some(new_chat_id) = migrated_chat_id(&error) else {
                    continue;
                };
                state
                    .store
                    .migrate_chat(listed_chat.chat_id, new_chat_id)
                    .await
                    .map_err(ApiError::store)?;
                let Ok(member) = state
                    .bot
                    .get_chat_member(ChatId(new_chat_id), UserId(auth.user.id))
                    .await
                else {
                    continue;
                };
                (new_chat_id, member)
            }
        };
        if !member.is_privileged() {
            continue;
        }
        let Ok(telegram_chat) = state.bot.get_chat(ChatId(chat_id)).await else {
            continue;
        };
        index_telegram_member(&state.store, chat_id, &telegram_chat, &member).await?;
        let Some(chat) = state
            .store
            .managed_chat(chat_id)
            .await
            .map_err(ApiError::store)?
        else {
            continue;
        };
        if !allowed.iter().any(|item| item.chat_id == chat_id) {
            allowed.push(chat);
        }
    }
    Ok(ok(json!({ "items": allowed })))
}

async fn get_chat(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let chat = state
        .store
        .managed_chat(chat_id)
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("not_found", "Guruh topilmadi"))?;
    Ok(ok(json!(chat)))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    limit: Option<i64>,
}

async fn get_members(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    if let Some(user_id) = telegram_user_id_query(query.q.as_deref())
        && state
            .store
            .member(chat_id, user_id)
            .await
            .map_err(ApiError::store)?
            .is_none()
    {
        match state
            .bot
            .get_chat_member(ChatId(chat_id), UserId(user_id))
            .await
        {
            Ok(member) => {
                let chat = state
                    .store
                    .managed_chat(chat_id)
                    .await
                    .map_err(ApiError::store)?
                    .ok_or_else(|| ApiError::not_found("not_found", "Guruh topilmadi"))?;
                state
                    .store
                    .upsert_member(&MemberUpsert {
                        chat_id,
                        chat_title: &chat.title,
                        chat_username: chat.username.as_deref(),
                        chat_type: &chat.chat_type,
                        user_id: member.user.id.0,
                        username: member.user.username.as_deref(),
                        first_name: &member.user.first_name,
                        last_name: member.user.last_name.as_deref(),
                        is_bot: member.user.is_bot,
                        is_admin: Some(member.is_privileged()),
                        status: telegram_member_status(&member.kind),
                    })
                    .await
                    .map_err(ApiError::store)?;
            }
            Err(error) => {
                tracing::debug!(%error, chat_id, user_id, "Telegram ID bo'yicha a'zo topilmadi");
            }
        }
    }
    let mut members = state
        .store
        .members(chat_id, query.q.as_deref(), query.limit.unwrap_or(50))
        .await
        .map_err(ApiError::store)?;
    if members.is_empty() {
        if sync_chat_administrators(&state, chat_id).await.is_err() {
            tracing::warn!(
                chat_id,
                "Telegram administratorlarini a'zolar indeksiga sinxronlab bo'lmadi"
            );
        }
        members = state
            .store
            .members(chat_id, query.q.as_deref(), query.limit.unwrap_or(50))
            .await
            .map_err(ApiError::store)?;
    }
    Ok(ok(json!({ "items": members })))
}

async fn get_member(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((chat_id, user_id)): Path<(i64, u64)>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let member = state
        .store
        .member(chat_id, user_id)
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("not_found", "A'zo topilmadi"))?;
    Ok(ok(json!(member)))
}

#[derive(Debug, Deserialize)]
struct ModerationRequest {
    target_user_id: u64,
    reason: Option<String>,
    duration_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ModerationResponse {
    target_user_id: u64,
    action: String,
    warning_count: Option<u16>,
    warning_limit: Option<u16>,
    automatic_sanction: Option<String>,
    duration_secs: Option<u64>,
}

async fn get_warnings(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((chat_id, user_id)): Path<(i64, u64)>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let count = state
        .app
        .store
        .warning_count(chat_id, user_id)
        .await
        .map_err(ApiError::store)?;
    let settings = state
        .app
        .store
        .settings(chat_id)
        .await
        .map_err(ApiError::store)?;
    Ok(ok(
        json!({ "target_user_id": user_id, "count": count, "limit": settings.warn_limit }),
    ))
}

async fn moderation_warn(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let target = ensure_target(&state, chat_id, request.target_user_id).await?;
    let reason = request
        .reason
        .as_deref()
        .unwrap_or("Mini App orqali warning");
    if reason.chars().count() > 1000 {
        return Err(ApiError::bad_request(
            "invalid_limit",
            "Sabab 1000 belgidan oshmasligi kerak",
        ));
    }
    let warning = state
        .app
        .store
        .add_warning(chat_id, request.target_user_id, Some(actor.user.id), reason)
        .await
        .map_err(ApiError::store)?;
    let mut automatic_sanction = None;
    if warning.reached_limit {
        let settings = state
            .app
            .store
            .settings(chat_id)
            .await
            .map_err(ApiError::store)?;
        commands::execute_sanction(
            &state.bot,
            ChatId(chat_id),
            UserId(request.target_user_id),
            settings.warn_action,
            settings.mute_duration_secs,
        )
        .await
        .map_err(ApiError::telegram)?;
        sync_member_status(
            &state,
            chat_id,
            request.target_user_id,
            settings.warn_action,
        )
        .await?;
        state
            .app
            .store
            .clear_warnings(chat_id, request.target_user_id)
            .await
            .map_err(ApiError::store)?;
        automatic_sanction = Some(settings.warn_action.as_str().to_owned());
    }
    let notice = warning_notice(
        &target.user.first_name,
        warning.count,
        warning.limit,
        reason,
        automatic_sanction.as_deref(),
    );
    let notice_message = match state.bot.send_message(ChatId(chat_id), notice).await {
        Ok(message) => message,
        Err(error) => {
            write_api_audit(&state, chat_id, actor.user.id, request.target_user_id, "warn", Some(reason), "failed", None, json!({"warning_persisted":true,"warning_count":warning.count,"warning_limit":warning.limit,"automatic_sanction":automatic_sanction,"notification_error":error.to_string()})).await;
            return Err(ApiError::telegram(error));
        }
    };
    write_api_audit(&state, chat_id, actor.user.id, request.target_user_id, "warn", Some(reason), "success", None, json!({"warning_count":warning.count,"warning_limit":warning.limit,"automatic_sanction":automatic_sanction,"notification_message_id":notice_message.id.0})).await;
    Ok(ok(json!(ModerationResponse {
        target_user_id: request.target_user_id,
        action: "warn".into(),
        warning_count: Some(warning.count),
        warning_limit: Some(warning.limit),
        automatic_sanction,
        duration_secs: None
    })))
}

async fn moderation_unwarn(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let removed = state
        .app
        .store
        .clear_warnings(chat_id, request.target_user_id)
        .await
        .map_err(ApiError::store)?;
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        request.target_user_id,
        "unwarn",
        request.reason.as_deref(),
        "success",
        None,
        json!({"removed":removed}),
    )
    .await;
    Ok(ok(
        json!({ "target_user_id": request.target_user_id, "action":"unwarn", "removed":removed }),
    ))
}

async fn moderation_mute(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    ensure_target(&state, chat_id, request.target_user_id).await?;
    let settings = state
        .app
        .store
        .settings(chat_id)
        .await
        .map_err(ApiError::store)?;
    let duration = request.duration_secs.unwrap_or(settings.mute_duration_secs);
    validate_duration(duration)?;
    let result = commands::execute_sanction(
        &state.bot,
        ChatId(chat_id),
        UserId(request.target_user_id),
        Sanction::Mute,
        duration,
    )
    .await;
    audit_telegram_result(
        &state,
        chat_id,
        actor.user.id,
        &request,
        "mute",
        Some(duration),
        &result,
    )
    .await;
    result.map_err(ApiError::telegram)?;
    sync_member_status(&state, chat_id, request.target_user_id, Sanction::Mute).await?;
    Ok(ok(json!(ModerationResponse {
        target_user_id: request.target_user_id,
        action: "mute".into(),
        warning_count: None,
        warning_limit: None,
        automatic_sanction: None,
        duration_secs: Some(duration)
    })))
}

async fn moderation_unmute(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let chat = state
        .bot
        .get_chat(ChatId(chat_id))
        .await
        .map_err(ApiError::telegram)?;
    let permissions = chat.permissions().unwrap_or_else(ChatPermissions::all);
    let result = state
        .bot
        .restrict_chat_member(ChatId(chat_id), UserId(request.target_user_id), permissions)
        .await
        .map(|_| ());
    audit_telegram_result(
        &state,
        chat_id,
        actor.user.id,
        &request,
        "unmute",
        None,
        &result,
    )
    .await;
    result.map_err(ApiError::telegram)?;
    state
        .store
        .update_member_status(chat_id, request.target_user_id, "member")
        .await
        .map_err(ApiError::store)?;
    Ok(ok(
        json!({ "target_user_id":request.target_user_id, "action":"unmute" }),
    ))
}

async fn moderation_ban(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    ensure_target(&state, chat_id, request.target_user_id).await?;
    let result = state
        .bot
        .ban_chat_member(ChatId(chat_id), UserId(request.target_user_id))
        .await
        .map(|_| ());
    audit_telegram_result(
        &state,
        chat_id,
        actor.user.id,
        &request,
        "ban",
        None,
        &result,
    )
    .await;
    result.map_err(ApiError::telegram)?;
    sync_member_status(&state, chat_id, request.target_user_id, Sanction::Ban).await?;
    Ok(ok(
        json!({ "target_user_id":request.target_user_id, "action":"ban" }),
    ))
}

async fn moderation_unban(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<ModerationRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let result = state
        .bot
        .unban_chat_member(ChatId(chat_id), UserId(request.target_user_id))
        .only_if_banned(true)
        .await
        .map(|_| ());
    audit_telegram_result(
        &state,
        chat_id,
        actor.user.id,
        &request,
        "unban",
        None,
        &result,
    )
    .await;
    result.map_err(ApiError::telegram)?;
    state
        .store
        .update_member_status(chat_id, request.target_user_id, "left")
        .await
        .map_err(ApiError::store)?;
    Ok(ok(
        json!({ "target_user_id":request.target_user_id, "action":"unban" }),
    ))
}

async fn ensure_target(state: &ApiState, chat_id: i64, target_id: u64) -> ApiResult<ChatMember> {
    let member = state
        .bot
        .get_chat_member(ChatId(chat_id), UserId(target_id))
        .await
        .map_err(ApiError::telegram)?;
    if member.user.is_bot {
        return Err(ApiError::bad_request(
            "target_is_bot",
            "Botga bu amalni bajarib bo'lmaydi",
        ));
    }
    if member.is_privileged() {
        return Err(ApiError::bad_request(
            "target_is_admin",
            "Administratorga bu amalni bajarib bo'lmaydi",
        ));
    }
    Ok(member)
}

fn warning_notice(
    first_name: &str,
    count: u16,
    limit: u16,
    reason: &str,
    automatic_sanction: Option<&str>,
) -> String {
    match automatic_sanction {
        Some(sanction) => format!(
            "⚠️ {first_name} warning limitiga yetdi ({count}/{limit}). Avtomatik jazo: {sanction}.\nSabab: {reason}"
        ),
        None => format!("⚠️ {first_name} ogohlantirildi ({count}/{limit}).\nSabab: {reason}"),
    }
}

async fn get_settings(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    Ok(ok(json!(
        state
            .app
            .store
            .settings(chat_id)
            .await
            .map_err(ApiError::store)?
    )))
}

#[derive(Debug, Deserialize, Serialize)]
struct SettingsPatch {
    flood_limit: Option<u16>,
    flood_window_secs: Option<u64>,
    flood_action: Option<String>,
    warn_limit: Option<u16>,
    warn_action: Option<String>,
    mute_duration_secs: Option<u64>,
    welcome_enabled: Option<bool>,
    welcome_template: Option<String>,
    rules: Option<String>,
}

async fn patch_settings(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(patch): Json<SettingsPatch>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let before = state
        .app
        .store
        .settings(chat_id)
        .await
        .map_err(ApiError::store)?;
    let mut after = before.clone();
    if let Some(value) = patch.flood_limit {
        if value != 0 && !(3..=100).contains(&value) {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "flood_limit 0 yoki 3..100 bo'lishi kerak",
            ));
        }
        after.flood_limit = value;
    }
    if let Some(value) = patch.flood_window_secs {
        if !(1..=3600).contains(&value) {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "flood_window_secs 1..3600 bo'lishi kerak",
            ));
        }
        after.flood_window_secs = value;
    }
    if let Some(value) = patch.warn_limit {
        if !(1..=100).contains(&value) {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "warn_limit 1..100 bo'lishi kerak",
            ));
        }
        after.warn_limit = value;
    }
    if let Some(value) = patch.mute_duration_secs {
        validate_duration(value)?;
        after.mute_duration_secs = value;
    }
    if let Some(value) = patch.flood_action.as_deref() {
        after.flood_action = value
            .parse()
            .map_err(|message| ApiError::bad_request("invalid_limit", message))?;
    }
    if let Some(value) = patch.warn_action.as_deref() {
        let action: Sanction = value
            .parse()
            .map_err(|message| ApiError::bad_request("invalid_limit", message))?;
        if !matches!(action, Sanction::Mute | Sanction::Ban) {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "warn_action mute yoki ban bo'lishi kerak",
            ));
        }
        after.warn_action = action;
    }
    if let Some(value) = patch.welcome_enabled {
        after.welcome_enabled = value;
    }
    if let Some(value) = patch.welcome_template.as_ref() {
        if value.chars().count() > 3500 {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "welcome_template 3500 belgidan oshmasligi kerak",
            ));
        }
        after.welcome_template = value.clone();
    }
    if let Some(value) = patch.rules.as_ref() {
        if value.chars().count() > 3500 {
            return Err(ApiError::bad_request(
                "invalid_limit",
                "rules 3500 belgidan oshmasligi kerak",
            ));
        }
        after.rules = value.clone();
    }
    state
        .store
        .replace_settings(&after)
        .await
        .map_err(ApiError::store)?;
    state.app.invalidate_policy(chat_id);
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        actor.user.id,
        "settings_update",
        None,
        "success",
        None,
        json!({"before":before,"patch":patch,"after":after}),
    )
    .await;
    Ok(ok(json!(after)))
}

async fn get_blocklist(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let items = state
        .store
        .blocklist(chat_id, query.q.as_deref(), query.limit.unwrap_or(100))
        .await
        .map_err(ApiError::store)?;
    Ok(ok(json!({"items":items})))
}

#[derive(Debug, Deserialize)]
struct BlocklistCreate {
    term: String,
}
async fn add_blocklist(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Json(request): Json<BlocklistCreate>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let term = request.term.trim();
    if term.is_empty() || term.chars().count() > 200 {
        return Err(ApiError::bad_request(
            "invalid_limit",
            "Ibora 1..200 belgi bo'lishi kerak",
        ));
    }
    let entry = state
        .store
        .add_blocklist_entry(chat_id, term)
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::conflict("already_exists", "Ibora blocklistda mavjud"))?;
    state.app.invalidate_policy(chat_id);
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        actor.user.id,
        "blocklist_add",
        Some(term),
        "success",
        None,
        json!({"term_id":entry.id}),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({"data":entry}))))
}

async fn delete_blocklist(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((chat_id, term_id)): Path<(i64, i64)>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    let term = state
        .store
        .delete_blocklist_entry(chat_id, term_id)
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("not_found", "Blocklist iborasi topilmadi"))?;
    state.app.invalidate_policy(chat_id);
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        actor.user.id,
        "blocklist_remove",
        Some(&term),
        "success",
        None,
        json!({"term_id":term_id}),
    )
    .await;
    Ok(ok(json!({"deleted":true,"term_id":term_id})))
}

async fn get_audit(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Query(mut filter): Query<AuditFilter>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    validate_audit_filter(&filter)?;
    filter.limit = Some(filter.limit.unwrap_or(50).clamp(1, 200));
    let items = state
        .store
        .audit_records(chat_id, &filter)
        .await
        .map_err(ApiError::store)?;
    let next_cursor = items.last().map(|item| item.id);
    Ok(ok(json!({"items":items,"next_cursor":next_cursor})))
}

async fn get_audit_stats(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    Ok(ok(json!(
        state
            .store
            .audit_stats(chat_id)
            .await
            .map_err(ApiError::store)?
    )))
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    format: Option<String>,
    action: Option<String>,
    source: Option<String>,
    target_user_id: Option<u64>,
    q: Option<String>,
    from: Option<chrono::DateTime<Utc>>,
    to: Option<chrono::DateTime<Utc>>,
}
async fn export_audit(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Response> {
    require_admin(&state, &headers, chat_id).await?;
    if query.format.as_deref().unwrap_or("csv") != "csv" {
        return Err(ApiError::bad_request(
            "invalid_format",
            "Faqat csv qo'llanadi",
        ));
    }
    let filter = AuditFilter {
        action: query.action,
        source: query.source,
        target_user_id: query.target_user_id,
        q: query.q,
        from: query.from,
        to: query.to,
        cursor: None,
        limit: Some(10_000),
    };
    validate_audit_filter(&filter)?;
    let items = state
        .store
        .audit_records(chat_id, &filter)
        .await
        .map_err(ApiError::store)?;
    let mut csv = String::from(
        "id,created_at,action,source,status,actor_id,target_id,duration_secs,reason\n",
    );
    for item in items {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            item.id,
            item.created_at.to_rfc3339(),
            csv_cell(&item.action),
            csv_cell(&item.source),
            csv_cell(&item.status),
            item.actor_id.map(|v| v.to_string()).unwrap_or_default(),
            item.target_id,
            item.duration_secs
                .map(|v| v.to_string())
                .unwrap_or_default(),
            csv_cell(item.reason.as_deref().unwrap_or(""))
        ));
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=cheklabot-audit-{chat_id}.csv"),
        )
        .body(Body::from(csv))
        .map_err(|error| ApiError::internal(error.to_string()))
}

async fn get_modules(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let modules = state
        .store
        .modules(chat_id)
        .await
        .map_err(ApiError::store)?;
    Ok(ok(
        json!({"items":modules,"total":modules.len(),"enabled":modules.iter().filter(|m|m.enabled).count()}),
    ))
}

#[derive(Debug, Deserialize)]
struct ModulePatch {
    enabled: Option<bool>,
    config: Option<Value>,
}
async fn patch_module(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((chat_id, module_key)): Path<(i64, String)>,
    Json(request): Json<ModulePatch>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    if request.enabled == Some(true)
        && matches!(module_key.as_str(), "captcha" | "anti_raid" | "reports")
    {
        return Err(ApiError::conflict(
            "module_unavailable",
            "Bu modul hali implementatsiya qilinmagan",
        ));
    }
    if request.enabled == Some(false)
        && matches!(
            module_key.as_str(),
            "telegram_permissions"
                | "database"
                | "admin_auth"
                | "audit"
                | "member_index"
                | "incident_response"
        )
    {
        return Err(ApiError::conflict(
            "module_required",
            "Bu asosiy modulni o'chirib bo'lmaydi",
        ));
    }
    if let Some(enabled) = request.enabled {
        if matches!(module_key.as_str(), "anti_flood" | "welcome") {
            let mut settings = state
                .app
                .store
                .settings(chat_id)
                .await
                .map_err(ApiError::store)?;
            match module_key.as_str() {
                "anti_flood" => {
                    settings.flood_limit = if enabled {
                        settings.flood_limit.max(8)
                    } else {
                        0
                    };
                }
                "welcome" => settings.welcome_enabled = enabled,
                _ => unreachable!(),
            }
            state
                .store
                .replace_settings(&settings)
                .await
                .map_err(ApiError::store)?;
            state.app.invalidate_policy(chat_id);
        }
    }
    let effective_config = if module_key == "link_filter" && request.config.is_none() {
        Some(json!({
            "action": "delete",
            "admin_exempt": true,
            "links": true,
            "mentions": true
        }))
    } else {
        request.config.clone()
    };
    let module = state
        .store
        .update_module(
            chat_id,
            &module_key,
            request.enabled,
            effective_config.clone(),
        )
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("not_found", "Modul topilmadi"))?;
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        actor.user.id,
        "module_update",
        Some(&module_key),
        "success",
        None,
        json!({"enabled":request.enabled,"config":effective_config}),
    )
    .await;
    Ok(ok(json!(module)))
}

#[derive(Debug, Deserialize)]
struct IncidentQuery {
    status: Option<String>,
    cursor: Option<i64>,
    limit: Option<i64>,
}

async fn get_incidents(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
    Query(query): Query<IncidentQuery>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    if let Some(status) = query.status.as_deref() {
        if !matches!(status, "detected" | "acknowledged" | "resolved") {
            return Err(ApiError::bad_request(
                "invalid_status",
                "Incident statusi noto'g'ri",
            ));
        }
    }
    let items = state
        .store
        .incidents(
            chat_id,
            query.status.as_deref(),
            query.cursor,
            query.limit.unwrap_or(50),
        )
        .await
        .map_err(ApiError::store)?;
    let next_cursor = items.last().map(|item| item.id);
    Ok(ok(json!({"items":items,"next_cursor":next_cursor})))
}

#[derive(Debug, Deserialize)]
struct IncidentPatch {
    status: String,
}

async fn patch_incident(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((chat_id, incident_id)): Path<(i64, i64)>,
    Json(request): Json<IncidentPatch>,
) -> ApiResult<impl IntoResponse> {
    let actor = require_admin(&state, &headers, chat_id).await?;
    if !matches!(request.status.as_str(), "acknowledged" | "resolved") {
        return Err(ApiError::bad_request(
            "invalid_status",
            "Incident faqat acknowledged yoki resolved qilinadi",
        ));
    }
    let incident = state
        .store
        .update_incident_status(chat_id, incident_id, &request.status, actor.user.id)
        .await
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("not_found", "Incident topilmadi"))?;
    write_api_audit(
        &state,
        chat_id,
        actor.user.id,
        actor.user.id,
        if request.status == "resolved" {
            "incident_resolve"
        } else {
            "incident_acknowledge"
        },
        None,
        "success",
        None,
        json!({"incident_id":incident_id,"status":request.status}),
    )
    .await;
    Ok(ok(json!(incident)))
}

async fn get_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let telegram = refresh_health(&state, chat_id).await?;
    let database_ready = state.app.store.healthcheck().await.is_ok();
    Ok(ok(
        json!({"chat_id":chat_id,"database_ready":database_ready,"bot_status":if database_ready&&telegram.bot_admin&&telegram.can_delete_messages&&telegram.can_restrict_members{"active"}else{"degraded"},"telegram":telegram,"checked_at":Utc::now()}),
    ))
}

async fn get_overview(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(chat_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    require_admin(&state, &headers, chat_id).await?;
    let health = refresh_health(&state, chat_id).await?;
    let modules = state
        .store
        .modules(chat_id)
        .await
        .map_err(ApiError::store)?;
    let stats = state
        .store
        .audit_stats(chat_id)
        .await
        .map_err(ApiError::store)?;
    let incidents = state
        .store
        .incident_metrics(chat_id)
        .await
        .map_err(ApiError::store)?;
    let recent = state
        .store
        .audit_records(
            chat_id,
            &AuditFilter {
                limit: Some(10),
                ..Default::default()
            },
        )
        .await
        .map_err(ApiError::store)?;
    let breakdown = score_breakdown(&modules);
    let protection_score: u16 = breakdown.iter().map(|item| item.earned).sum();
    let database_ready = state.app.store.healthcheck().await.is_ok();
    let bot_active = database_ready
        && health.bot_admin
        && health.can_delete_messages
        && health.can_restrict_members;
    Ok(ok(
        json!({"protection_score":protection_score,"protection_score_breakdown":breakdown,
        "enabled_modules":modules.iter().filter(|m|m.enabled).count(),"total_modules":modules.len(),
        "auto_blocks_today":stats.auto_blocks_today,"warnings_today":stats.warnings_today,
        "average_response_seconds":incidents.average_response_seconds,"open_incidents":incidents.open_incidents,
        "bot_status":if bot_active{"active"}else{"degraded"},"last_check_at":Utc::now(),
        "recent_activity":recent}),
    ))
}

async fn refresh_health(
    state: &ApiState,
    chat_id: i64,
) -> ApiResult<qalqon_storage::RuntimeHealth> {
    let member = state
        .bot
        .get_chat_member(ChatId(chat_id), state.me.id)
        .await
        .map_err(ApiError::telegram)?;
    let (present, admin, delete, restrict) = match &member.kind {
        ChatMemberKind::Owner(_) => (true, true, true, true),
        ChatMemberKind::Administrator(value) => (
            true,
            true,
            value.can_delete_messages,
            value.can_restrict_members,
        ),
        kind => (kind.is_present(), false, false, false),
    };
    state
        .store
        .update_runtime(chat_id, present, admin, delete, restrict, restrict)
        .await
        .map_err(ApiError::store)?;
    state
        .store
        .runtime_health(chat_id)
        .await
        .map_err(ApiError::store)
}

#[derive(Debug, Serialize)]
struct ScoreItem {
    module_key: String,
    weight: u16,
    earned: u16,
    reason: String,
}
fn score_breakdown(modules: &[ProtectionModule]) -> Vec<ScoreItem> {
    const WEIGHTS: [(&str, u16); 15] = [
        ("telegram_permissions", 20),
        ("database", 12),
        ("admin_auth", 8),
        ("anti_flood", 10),
        ("warning_policy", 8),
        ("blocklist", 8),
        ("welcome", 3),
        ("rules", 3),
        ("audit", 8),
        ("member_index", 5),
        ("captcha", 5),
        ("anti_raid", 4),
        ("link_filter", 3),
        ("reports", 2),
        ("incident_response", 1),
    ];
    WEIGHTS
        .into_iter()
        .map(|(key, weight)| {
            let module = modules.iter().find(|m| m.module_key == key);
            let earned = module
                .filter(|m| m.enabled && m.healthy && m.configured)
                .map_or(0, |_| weight);
            ScoreItem {
                module_key: key.into(),
                weight,
                earned,
                reason: if earned == weight {
                    "enabled_healthy_configured".into()
                } else {
                    "disabled_unhealthy_or_unconfigured".into()
                },
            }
        })
        .collect()
}

fn validate_duration(value: u64) -> ApiResult<()> {
    if !(30..=31_536_000).contains(&value) {
        Err(ApiError::bad_request(
            "invalid_duration",
            "duration 30..31536000 sekund bo'lishi kerak",
        ))
    } else {
        Ok(())
    }
}

fn telegram_user_id_query(query: Option<&str>) -> Option<u64> {
    let value = query?.trim();
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn migrated_chat_id(error: &RequestError) -> Option<i64> {
    match error {
        RequestError::MigrateToChatId(chat_id) => Some(chat_id.0),
        _ => None,
    }
}

fn telegram_member_status(kind: &ChatMemberKind) -> &'static str {
    match kind {
        ChatMemberKind::Owner(_) => "creator",
        ChatMemberKind::Administrator(_) => "administrator",
        ChatMemberKind::Member(_) => "member",
        ChatMemberKind::Restricted(_) => "restricted",
        ChatMemberKind::Left => "left",
        ChatMemberKind::Banned(_) => "kicked",
    }
}
fn validate_audit_filter(filter: &AuditFilter) -> ApiResult<()> {
    if let Some(source) = filter.source.as_deref() {
        if !matches!(source, "auto" | "admin") {
            return Err(ApiError::bad_request(
                "invalid_filter",
                "source auto yoki admin bo'lishi kerak",
            ));
        }
    }
    Ok(())
}
fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

// Audit call sites deliberately spell out every security-relevant field.
#[allow(clippy::too_many_arguments)]
async fn write_api_audit(
    state: &ApiState,
    chat_id: i64,
    actor: u64,
    target: u64,
    action: &str,
    reason: Option<&str>,
    status: &str,
    duration: Option<u64>,
    metadata: Value,
) {
    if let Err(error) = state
        .store
        .rich_audit(&RichAuditEvent {
            chat_id,
            actor_id: Some(actor),
            target_id: target,
            action,
            reason,
            source: "admin",
            status,
            duration_secs: duration,
            telegram_message_id: None,
            telegram_update_id: None,
            metadata,
        })
        .await
    {
        tracing::warn!(%error,"API audit yozilmadi");
    }
}
async fn audit_telegram_result<T, E: std::fmt::Display>(
    state: &ApiState,
    chat_id: i64,
    actor: u64,
    request: &ModerationRequest,
    action: &str,
    duration: Option<u64>,
    result: &Result<T, E>,
) {
    let (status, error) = match result {
        Ok(_) => ("success", None),
        Err(error) => ("failed", Some(error.to_string())),
    };
    write_api_audit(
        state,
        chat_id,
        actor,
        request.target_user_id,
        action,
        request.reason.as_deref(),
        status,
        duration,
        json!({"telegram_error":error}),
    )
    .await;
}

async fn sync_member_status(
    state: &ApiState,
    chat_id: i64,
    target_user_id: u64,
    sanction: Sanction,
) -> ApiResult<()> {
    let Some(status) = commands::sanction_member_status(sanction) else {
        return Ok(());
    };
    state
        .store
        .update_member_status(chat_id, target_user_id, status)
        .await
        .map_err(ApiError::store)?;
    Ok(())
}

fn ok(data: Value) -> Json<Value> {
    Json(json!({"data":data}))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }
    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }
    fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }
    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }
    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }
    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
    fn store(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error,"API storage xatosi");
        Self::internal("Ma'lumotlar bazasi xatosi")
    }
    fn telegram(error: impl std::fmt::Display) -> Self {
        tracing::warn!(%error,"Telegram API xatosi");
        Self::new(
            StatusCode::BAD_GATEWAY,
            "telegram_error",
            "Telegram amali bajarilmadi",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_notice_is_visible_and_describes_the_result() {
        assert_eq!(
            warning_notice("Ali", 1, 3, "Takroriy reklama", None),
            "⚠️ Ali ogohlantirildi (1/3).\nSabab: Takroriy reklama"
        );
        assert_eq!(
            warning_notice("Ali", 3, 3, "Takroriy reklama", Some("mute")),
            "⚠️ Ali warning limitiga yetdi (3/3). Avtomatik jazo: mute.\nSabab: Takroriy reklama"
        );
    }

    #[test]
    fn score_weights_sum_to_one_hundred() {
        let modules = Vec::new();
        let score = score_breakdown(&modules);
        assert_eq!(score.iter().map(|item| item.weight).sum::<u16>(), 100);
    }

    #[test]
    fn csv_escaping_is_safe() {
        assert_eq!(csv_cell("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn detects_exact_telegram_id_queries() {
        assert_eq!(telegram_user_id_query(Some(" 884201 ")), Some(884201));
        assert_eq!(telegram_user_id_query(Some("@alisher")), None);
        assert_eq!(telegram_user_id_query(Some("884201 ali")), None);
    }

    #[test]
    fn detects_stale_group_id_from_telegram_error() {
        let error = teloxide::RequestError::MigrateToChatId(ChatId(-1004487463600));
        assert_eq!(migrated_chat_id(&error), Some(-1004487463600));
    }

    #[test]
    fn validates_signed_init_data_and_rejects_tampering() {
        let token = "123456789:test-token";
        let auth_date = Utc::now().timestamp();
        let user = r#"{"id":42,"first_name":"Admin","username":"admin"}"#;
        let mut values = BTreeMap::from([
            ("auth_date".to_owned(), auth_date.to_string()),
            ("query_id".to_owned(), "query-1".to_owned()),
            ("user".to_owned(), user.to_owned()),
        ]);
        let check_string = values
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(token.as_bytes());
        let secret = secret_mac.finalize().into_bytes();
        let mut data_mac = HmacSha256::new_from_slice(&secret).unwrap();
        data_mac.update(check_string.as_bytes());
        values.insert("hash".into(), hex::encode(data_mac.finalize().into_bytes()));
        let signed = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(values.iter())
            .finish();
        let auth = validate_init_data(&signed, token, Duration::from_secs(60)).unwrap();
        assert_eq!(auth.user.id, 42);

        let tampered = signed.replace("Admin", "Attacker");
        assert_eq!(
            validate_init_data(&tampered, token, Duration::from_secs(60))
                .unwrap_err()
                .code,
            "invalid_init_data"
        );
    }

    #[test]
    fn validates_init_data_with_telegram_signature_field() {
        let token = "123456789:test-token";
        let auth_date = Utc::now().timestamp();
        let mut values = BTreeMap::from([
            ("auth_date".to_owned(), auth_date.to_string()),
            ("query_id".to_owned(), "query-2".to_owned()),
            (
                "signature".to_owned(),
                "telegram-third-party-signature".to_owned(),
            ),
            (
                "user".to_owned(),
                r#"{"id":43,"first_name":"Admin"}"#.to_owned(),
            ),
        ]);
        let check_string = values
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(token.as_bytes());
        let secret = secret_mac.finalize().into_bytes();
        let mut data_mac = HmacSha256::new_from_slice(&secret).unwrap();
        data_mac.update(check_string.as_bytes());
        values.insert("hash".into(), hex::encode(data_mac.finalize().into_bytes()));
        let signed = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(values.iter())
            .finish();

        let auth = validate_init_data(&signed, token, Duration::from_secs(60)).unwrap();
        assert_eq!(auth.user.id, 43);
    }
}
