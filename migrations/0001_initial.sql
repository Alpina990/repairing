CREATE TABLE IF NOT EXISTS chat_settings (
    chat_id BIGINT PRIMARY KEY,
    flood_limit INTEGER NOT NULL DEFAULT 8 CHECK (flood_limit BETWEEN 0 AND 100),
    flood_window_secs INTEGER NOT NULL DEFAULT 10 CHECK (flood_window_secs BETWEEN 1 AND 3600),
    flood_action TEXT NOT NULL DEFAULT 'mute' CHECK (flood_action IN ('delete','warn','mute','ban')),
    warn_limit INTEGER NOT NULL DEFAULT 3 CHECK (warn_limit BETWEEN 1 AND 100),
    warn_action TEXT NOT NULL DEFAULT 'mute' CHECK (warn_action IN ('mute','ban')),
    mute_duration_secs INTEGER NOT NULL DEFAULT 3600 CHECK (mute_duration_secs BETWEEN 30 AND 31536000),
    welcome_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    welcome_template TEXT NOT NULL DEFAULT 'Xush kelibsiz, {first_name}! {chat_title} qoidalarini /rules orqali o''qing.',
    rules TEXT NOT NULL DEFAULT 'Guruh qoidalari hali administrator tomonidan kiritilmagan.',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS blocked_terms (
    chat_id BIGINT NOT NULL REFERENCES chat_settings(chat_id) ON DELETE CASCADE,
    term TEXT NOT NULL CHECK (char_length(trim(term)) BETWEEN 1 AND 200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chat_id, term)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_blocked_terms_case_insensitive
    ON blocked_terms(chat_id, lower(term));

CREATE TABLE IF NOT EXISTS warnings (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL REFERENCES chat_settings(chat_id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL,
    actor_id BIGINT,
    reason TEXT NOT NULL CHECK (char_length(reason) <= 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_warnings_chat_user ON warnings(chat_id, user_id);

CREATE TABLE IF NOT EXISTS moderation_audit (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL,
    actor_id BIGINT,
    target_id BIGINT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_audit_chat_created ON moderation_audit(chat_id, created_at DESC);
