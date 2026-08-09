export type Sanction = "delete" | "warn" | "mute" | "ban";

export interface MiniAppUser {
  id: number;
  first_name: string;
  last_name?: string | null;
  username?: string | null;
  photo_url?: string | null;
}

export interface MeResponse {
  user: MiniAppUser;
  auth_date: number;
}

export interface ManagedChat {
  chat_id: number;
  title: string;
  username?: string | null;
  chat_username?: string | null;
  chat_type: "group" | "supergroup";
  last_update_at?: string | null;
  updated_at?: string | null;
}

export interface ChatMember {
  chat_id: number;
  user_id: number;
  username?: string | null;
  first_name: string;
  last_name?: string | null;
  is_bot: boolean;
  is_admin?: boolean | null;
  status: string;
  last_seen_at?: string | null;
  updated_at?: string | null;
}

export interface ChatSettings {
  chat_id: number;
  flood_limit: number;
  flood_window_secs: number;
  flood_action: Sanction;
  warn_limit: number;
  warn_action: "mute" | "ban";
  mute_duration_secs: number;
  welcome_enabled: boolean;
  welcome_template: string;
  rules: string;
}

export interface ScoreItem {
  module_key: string;
  weight: number;
  earned: number;
  reason: string;
}

export interface ProtectionModule {
  module_key: string;
  title: string;
  enabled: boolean;
  healthy: boolean;
  configured: boolean;
  last_triggered_at?: string | null;
  config: Record<string, unknown>;
  updated_at?: string;
}

export interface AuditRecord {
  id: number;
  actor_id?: number | null;
  target_id: number;
  action: string;
  reason?: string | null;
  source: "auto" | "admin";
  status: "success" | "failed";
  duration_secs?: number | null;
  telegram_message_id?: number | null;
  telegram_update_id?: number | null;
  metadata?: Record<string, unknown>;
  created_at: string;
  actor_username?: string | null;
  target_username?: string | null;
  target_first_name?: string | null;
}

export interface Overview {
  protection_score: number;
  protection_score_breakdown: ScoreItem[];
  enabled_modules: number;
  total_modules: number;
  auto_blocks_today: number;
  warnings_today: number;
  average_response_seconds?: number | null;
  open_incidents: number;
  bot_status: "active" | "degraded";
  last_check_at: string;
  recent_activity: AuditRecord[];
}

export interface BlockedTerm {
  id: number;
  chat_id?: number;
  term: string;
  match_count: number;
  last_matched_at?: string | null;
  updated_at?: string | null;
}

export interface Incident {
  id: number;
  chat_id: number;
  incident_type: string;
  severity: "low" | "medium" | "high" | "critical";
  detected_at: string;
  acknowledged_at?: string | null;
  resolved_at?: string | null;
  resolved_by?: number | null;
  status: "detected" | "acknowledged" | "resolved";
  metadata: Record<string, unknown>;
}

export interface RuntimeHealth {
  chat_id: number;
  database_ready: boolean;
  bot_status: "active" | "degraded";
  checked_at: string;
  telegram: {
    bot_present: boolean;
    bot_admin: boolean;
    can_delete_messages: boolean;
    can_restrict_members: boolean;
    can_ban_members: boolean;
    last_update_at?: string | null;
  };
}

export interface ModerationPayload {
  target_user_id: number;
  reason?: string;
  duration_secs?: number;
}

export interface ModerationResult {
  target_user_id: number;
  action: string;
  warning_count?: number | null;
  warning_limit?: number | null;
  automatic_sanction?: string | null;
  duration_secs?: number | null;
  removed?: number;
}

export interface ApiErrorShape {
  code: string;
  message: string;
  status?: number;
}
