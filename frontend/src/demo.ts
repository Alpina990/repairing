import type { CheklaApi } from "./api";
import type { AuditRecord, BlockedTerm, ChatMember, ChatSettings, Incident, ProtectionModule } from "./types";

const now = Date.now();
const isoAgo = (seconds: number) => new Date(now - seconds * 1000).toISOString();

let settings: ChatSettings = {
  chat_id: -1001894202401,
  flood_limit: 8,
  flood_window_secs: 10,
  flood_action: "mute",
  warn_limit: 3,
  warn_action: "mute",
  mute_duration_secs: 3600,
  welcome_enabled: true,
  welcome_template: "Xush kelibsiz, {first_name}!\n{chat_title} qoidalarini /rules orqali o‘qing.",
  rules: "1. Hurmat bilan muloqot qiling\n2. Reklama va spam taqiqlanadi\n3. Mavzudan tashqari xabar yubormang",
};

const members: ChatMember[] = [
  { chat_id: settings.chat_id, user_id: 884201, username: "alisher", first_name: "Alisher", last_name: "Karimov", is_bot: false, is_admin: false, status: "active", last_seen_at: isoAgo(90) },
  { chat_id: settings.chat_id, user_id: 40128, username: "jasur", first_name: "Jasur", last_name: "Qodirov", is_bot: false, is_admin: false, status: "muted", last_seen_at: isoAgo(360) },
  { chat_id: settings.chat_id, user_id: 98731, username: "bekzod", first_name: "Bekzod", last_name: "Usmonov", is_bot: false, is_admin: false, status: "active", last_seen_at: isoAgo(840) },
];

let blockedTerms: BlockedTerm[] = [
  { id: 1, term: "reklama", match_count: 12, last_matched_at: isoAgo(840), updated_at: isoAgo(840) },
  { id: 2, term: "crypto signal", match_count: 7, last_matched_at: isoAgo(3600), updated_at: isoAgo(3600) },
  { id: 3, term: "t.me/+", match_count: 4, last_matched_at: isoAgo(7200), updated_at: isoAgo(7200) },
];

const audits: AuditRecord[] = [
  { id: 52, target_id: 40128, action: "mute", reason: "anti-flood · 14 xabar", source: "auto", status: "success", duration_secs: 3600, created_at: isoAgo(120), target_username: "jasur", target_first_name: "Jasur" },
  { id: 51, actor_id: 42001, target_id: 884201, action: "warn", reason: "takroriy reklama", source: "admin", status: "success", created_at: isoAgo(480), target_username: "alisher", target_first_name: "Alisher" },
  { id: 50, target_id: 98731, action: "blocklist_match", reason: "crypto signal", source: "auto", status: "success", created_at: isoAgo(840), target_username: "bekzod", target_first_name: "Bekzod" },
  { id: 49, actor_id: 42001, target_id: 77121, action: "ban", reason: "ommaviy reklama", source: "admin", status: "success", created_at: isoAgo(1920), target_username: "spam_account", target_first_name: "Spam" },
];

let incidents: Incident[] = [
  { id: 1042, chat_id: settings.chat_id, incident_type: "anti_flood", severity: "medium", detected_at: isoAgo(120), status: "detected", metadata: { user_id: 40128, message_id: 982 } },
  { id: 1041, chat_id: settings.chat_id, incident_type: "blocklist_match", severity: "medium", detected_at: isoAgo(840), acknowledged_at: isoAgo(600), status: "acknowledged", metadata: { user_id: 98731, term: "crypto signal" } },
  { id: 1040, chat_id: settings.chat_id, incident_type: "anti_flood", severity: "low", detected_at: isoAgo(4200), acknowledged_at: isoAgo(4000), resolved_at: isoAgo(3600), resolved_by: 42001, status: "resolved", metadata: { user_id: 77213 } },
];

const moduleRows: ProtectionModule[] = [
  ["telegram_permissions", "Telegram ruxsatlari", true, true, true],
  ["database", "PostgreSQL", true, true, true],
  ["admin_auth", "Admin autentifikatsiyasi", true, true, true],
  ["anti_flood", "Anti-flood", true, true, true],
  ["warning_policy", "Warning siyosati", true, true, true],
  ["blocklist", "Blocklist", true, true, true],
  ["welcome", "Welcome", true, true, true],
  ["rules", "Guruh qoidalari", true, true, true],
  ["audit", "Audit log", true, true, true],
  ["member_index", "A’zolar indeksi", true, true, true],
  ["incident_response", "Incident response", true, true, true],
  ["captcha", "CAPTCHA", false, true, false],
  ["anti_raid", "Anti-raid", false, true, false],
  ["link_filter", "Link filtri", false, true, false],
  ["reports", "Shikoyatlar", false, true, false],
].map(([module_key, title, enabled, healthy, configured]) => ({ module_key, title, enabled, healthy, configured, config: {} } as ProtectionModule));

const wait = async <T>(value: T): Promise<T> => {
  await new Promise((resolve) => setTimeout(resolve, 180));
  return structuredClone(value);
};

export function createDemoApi(): CheklaApi {
  return {
    getMe: () => wait({ user: { id: 42001, first_name: "Shohrux", username: "alpina" }, auth_date: Math.floor(now / 1000) }),
    getChats: () => wait({ items: [
      { chat_id: settings.chat_id, title: "Product Community", username: "product_community", chat_type: "supergroup", last_update_at: isoAgo(20) },
      { chat_id: -10011223344, title: "Frontend UZ", chat_type: "supergroup", last_update_at: isoAgo(120) },
    ] }),
    getOverview: () => wait({
      protection_score: 86,
      protection_score_breakdown: moduleRows.map((item) => ({ module_key: item.module_key, weight: item.module_key === "telegram_permissions" ? 20 : 5, earned: item.enabled && item.configured ? (item.module_key === "telegram_permissions" ? 20 : 5) : 0, reason: item.enabled ? "enabled_healthy_configured" : "disabled_unhealthy_or_unconfigured" })),
      enabled_modules: 11,
      total_modules: 15,
      auto_blocks_today: 24,
      warnings_today: 8,
      average_response_seconds: 240,
      open_incidents: incidents.filter((item) => item.status !== "resolved").length,
      bot_status: "active",
      last_check_at: new Date().toISOString(),
      recent_activity: audits,
    }),
    getMembers: (_chatId, q = "") => {
      const terms = q.trim().toLowerCase().split(/\s+/).map((term) => term.replace(/^@/, "")).filter(Boolean);
      return wait({ items: members.filter((member) => { const index = `${member.first_name} ${member.last_name ?? ""} ${member.username ?? ""} ${member.user_id}`.toLowerCase(); return terms.every((term) => index.includes(term)); }) });
    },
    getMember: async (_chatId, userId) => {
      const member = members.find((item) => item.user_id === userId);
      if (!member) throw new Error("A’zo topilmadi");
      return wait(member);
    },
    getWarnings: (_chatId, userId) => wait({ count: userId === 884201 ? 2 : 0, limit: settings.warn_limit }),
    moderate: async (_chatId, action, payload) => {
      await wait(null);
      return { target_user_id: payload.target_user_id, action, warning_count: action === "warn" ? 3 : undefined, warning_limit: action === "warn" ? settings.warn_limit : undefined, automatic_sanction: action === "warn" ? settings.warn_action : undefined, duration_secs: payload.duration_secs };
    },
    getSettings: () => wait(settings),
    patchSettings: async (_chatId, patch) => {
      settings = { ...settings, ...patch };
      return wait(settings);
    },
    getBlocklist: (_chatId, q = "") => wait({ items: blockedTerms.filter((item) => item.term.toLowerCase().includes(q.toLowerCase())) }),
    addBlockedTerm: async (_chatId, term) => {
      const item = { id: Math.max(0, ...blockedTerms.map((row) => row.id)) + 1, term, match_count: 0, updated_at: new Date().toISOString() };
      blockedTerms = [item, ...blockedTerms];
      return wait(item);
    },
    deleteBlockedTerm: async (_chatId, termId) => {
      blockedTerms = blockedTerms.filter((item) => item.id !== termId);
      await wait(null);
    },
    getAudit: (_chatId, filters = {}) => wait({ items: audits.filter((item) => (!filters.action || item.action === filters.action) && (!filters.source || item.source === filters.source) && (!filters.target_user_id || item.target_id === filters.target_user_id) && (!filters.q || `${item.target_username} ${item.target_first_name} ${item.reason}`.toLowerCase().includes(filters.q.toLowerCase()))), next_cursor: null }),
    exportAudit: () => wait(new Blob(["id,action,source,status\n52,mute,auto,success\n"], { type: "text/csv" })),
    getModules: () => wait({ items: moduleRows, total: moduleRows.length, enabled: moduleRows.filter((item) => item.enabled).length }),
    patchModule: async (_chatId, key, patch) => {
      const item = moduleRows.find((row) => row.module_key === key);
      if (!item) throw new Error("Modul topilmadi");
      if (patch.enabled !== undefined) item.enabled = patch.enabled;
      return wait(item);
    },
    getIncidents: (_chatId, status) => wait({ items: incidents.filter((item) => !status || item.status === status), next_cursor: null }),
    patchIncident: async (_chatId, id, status) => {
      const item = incidents.find((row) => row.id === id);
      if (!item) throw new Error("Incident topilmadi");
      item.status = status;
      if (status === "acknowledged") item.acknowledged_at = new Date().toISOString();
      if (status === "resolved") item.resolved_at = new Date().toISOString();
      incidents = [...incidents];
      return wait(item);
    },
    getHealth: () => wait({ chat_id: settings.chat_id, database_ready: true, bot_status: "active", checked_at: new Date().toISOString(), telegram: { bot_present: true, bot_admin: true, can_delete_messages: true, can_restrict_members: true, can_ban_members: true, last_update_at: isoAgo(20) } }),
  };
}
