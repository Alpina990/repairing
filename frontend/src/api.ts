import { telegramInitData } from "./telegram";
import type {
  AuditRecord,
  BlockedTerm,
  ChatMember,
  ChatSettings,
  Incident,
  ManagedChat,
  MeResponse,
  ModerationPayload,
  ModerationResult,
  Overview,
  ProtectionModule,
  RuntimeHealth,
} from "./types";

interface Envelope<T> {
  data: T;
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string };
}

export class ApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly status: number,
    public readonly requestId?: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export interface AuditFilters {
  action?: string;
  source?: "auto" | "admin";
  target_user_id?: number;
  q?: string;
  from?: string;
  to?: string;
  cursor?: number;
  limit?: number;
}

export interface CheklaApi {
  getMe(): Promise<MeResponse>;
  getChats(): Promise<{ items: ManagedChat[] }>;
  getOverview(chatId: number): Promise<Overview>;
  getMembers(chatId: number, q?: string): Promise<{ items: ChatMember[] }>;
  getMember(chatId: number, userId: number): Promise<ChatMember>;
  getWarnings(chatId: number, userId: number): Promise<{ count: number; limit: number }>;
  moderate(chatId: number, action: string, payload: ModerationPayload): Promise<ModerationResult>;
  getSettings(chatId: number): Promise<ChatSettings>;
  patchSettings(chatId: number, patch: Partial<ChatSettings>): Promise<ChatSettings>;
  getBlocklist(chatId: number, q?: string): Promise<{ items: BlockedTerm[] }>;
  addBlockedTerm(chatId: number, term: string): Promise<BlockedTerm>;
  deleteBlockedTerm(chatId: number, termId: number): Promise<void>;
  getAudit(chatId: number, filters?: AuditFilters): Promise<{ items: AuditRecord[]; next_cursor?: number | null }>;
  exportAudit(chatId: number, filters?: AuditFilters): Promise<Blob>;
  getModules(chatId: number): Promise<{ items: ProtectionModule[]; total: number; enabled: number }>;
  patchModule(chatId: number, key: string, patch: { enabled?: boolean; config?: Record<string, unknown> }): Promise<ProtectionModule>;
  getIncidents(chatId: number, status?: string): Promise<{ items: Incident[]; next_cursor?: number | null }>;
  patchIncident(chatId: number, id: number, status: "acknowledged" | "resolved"): Promise<Incident>;
  getHealth(chatId: number): Promise<RuntimeHealth>;
}

function queryString<T extends object>(values: T): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(values as Record<string, string | number | undefined>)) {
    if (value !== undefined && value !== "") query.set(key, String(value));
  }
  const value = query.toString();
  return value ? `?${value}` : "";
}

export function createHttpApi(base = import.meta.env.VITE_API_BASE || "/api"): CheklaApi {
  async function request<T>(path: string, init?: RequestInit): Promise<T> {
    const initData = telegramInitData();
    if (!initData) throw new ApiError("init_data_required", "Telegram Mini App orqali oching", 401);
    const response = await fetch(`${base}${path}`, {
      ...init,
      headers: {
        Authorization: `tma ${initData}`,
        "Content-Type": "application/json",
        ...init?.headers,
      },
    });
    if (!response.ok) {
      let payload: ErrorEnvelope = {};
      try {
        payload = (await response.json()) as ErrorEnvelope;
      } catch {
        // Non-JSON transport failures are normalized below.
      }
      throw new ApiError(
        payload.error?.code ?? "network_error",
        payload.error?.message ?? "Server bilan aloqa amalga oshmadi",
        response.status,
        response.headers.get("x-request-id") ?? undefined,
      );
    }
    const payload = (await response.json()) as Envelope<T>;
    return payload.data;
  }

  return {
    getMe: () => request("/me"),
    getChats: () => request("/chats"),
    getOverview: (chatId) => request(`/chats/${chatId}/overview`),
    getMembers: (chatId, q) => request(`/chats/${chatId}/members${queryString({ q, limit: 50 })}`),
    getMember: (chatId, userId) => request(`/chats/${chatId}/members/${userId}`),
    getWarnings: (chatId, userId) => request(`/chats/${chatId}/members/${userId}/warnings`),
    moderate: (chatId, action, payload) => request(`/chats/${chatId}/moderation/${action}`, { method: "POST", body: JSON.stringify(payload) }),
    getSettings: (chatId) => request(`/chats/${chatId}/settings`),
    patchSettings: (chatId, patch) => request(`/chats/${chatId}/settings`, { method: "PATCH", body: JSON.stringify(patch) }),
    getBlocklist: (chatId, q) => request(`/chats/${chatId}/blocklist${queryString({ q, limit: 100 })}`),
    addBlockedTerm: (chatId, term) => request(`/chats/${chatId}/blocklist`, { method: "POST", body: JSON.stringify({ term }) }),
    deleteBlockedTerm: async (chatId, termId) => {
      await request(`/chats/${chatId}/blocklist/${termId}`, { method: "DELETE" });
    },
    getAudit: (chatId, filters = {}) => request(`/chats/${chatId}/audit${queryString(filters)}`),
    exportAudit: async (chatId, filters = {}) => {
      const initData = telegramInitData();
      const response = await fetch(`${base}/chats/${chatId}/audit/export${queryString({ ...filters, format: "csv" })}`, {
        headers: { Authorization: `tma ${initData}` },
      });
      if (!response.ok) throw new ApiError("export_failed", "Audit eksport qilinmadi", response.status, response.headers.get("x-request-id") ?? undefined);
      return response.blob();
    },
    getModules: (chatId) => request(`/chats/${chatId}/modules`),
    patchModule: (chatId, key, patch) => request(`/chats/${chatId}/modules/${key}`, { method: "PATCH", body: JSON.stringify(patch) }),
    getIncidents: (chatId, status) => request(`/chats/${chatId}/incidents${queryString({ status, limit: 50 })}`),
    patchIncident: (chatId, id, status) => request(`/chats/${chatId}/incidents/${id}`, { method: "PATCH", body: JSON.stringify({ status }) }),
    getHealth: (chatId) => request(`/chats/${chatId}/health`),
  };
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return "Noma’lum xato yuz berdi";
}
