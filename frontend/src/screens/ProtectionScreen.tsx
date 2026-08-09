import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Database, ListChecks, Plus, Search, ShieldCheck, Trash2, UsersRound, WandSparkles } from "lucide-react";
import { errorMessage } from "../api";
import { useApp } from "../app-context";
import { Badge, Button, Card, ConfirmDialog, PageHeader, SearchField, SectionHeader, Segmented, StateView, StatusDot, Toast, Toggle } from "../components";
import { relativeTime } from "../format";
import type { BlockedTerm, ChatSettings, ProtectionModule } from "../types";

type ProtectionTab = "policy" | "content" | "modules";
const requiredModules = new Set(["telegram_permissions", "database", "admin_auth", "audit", "member_index", "incident_response"]);
const unavailableModules = new Set(["captcha", "anti_raid", "link_filter", "reports"]);

export function ProtectionScreen() {
  const [tab, setTab] = useState<ProtectionTab>("policy");
  return <div className="page-stack">
    <PageHeader title="Himoya" subtitle="Siyosat, kontent va tizim holati" />
    <Segmented<ProtectionTab> value={tab} onChange={setTab} label="Himoya bo‘limlari" options={[{ value: "policy", label: "Asosiy" }, { value: "content", label: "Kontent" }, { value: "modules", label: "Modullar" }]} />
    {tab === "policy" && <PolicyPanel />}
    {tab === "content" && <ContentPanel />}
    {tab === "modules" && <ModulesPanel />}
  </div>;
}

function PolicyPanel() {
  const { api, chatId } = useApp();
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({ queryKey: ["settings", chatId], queryFn: () => api.getSettings(chatId!), enabled: !!chatId });
  const [draft, setDraft] = useState<ChatSettings>();
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  useEffect(() => { if (settingsQuery.data) setDraft(settingsQuery.data); }, [settingsQuery.data]);
  const mutation = useMutation({ mutationFn: (value: ChatSettings) => api.patchSettings(chatId!, value), onSuccess: (value) => { setDraft(value); void queryClient.invalidateQueries({ queryKey: ["settings", chatId] }); void queryClient.invalidateQueries({ queryKey: ["overview", chatId] }); setToast({ message: "Himoya siyosati saqlandi", tone: "success" }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  if (!draft) return settingsQuery.error ? <StateView kind="error" message={errorMessage(settingsQuery.error)} /> : <StateView kind="loading" />;

  const set = <K extends keyof ChatSettings>(key: K, value: ChatSettings[K]) => setDraft((current) => current ? { ...current, [key]: value } : current);
  return <>
    <Card>
      <SectionHeader title="Anti-flood" meta="Sliding window" action={<Toggle label="Anti-flood" checked={draft.flood_limit > 0} onChange={(enabled) => set("flood_limit", enabled ? 8 : 0)} />} />
      <div className="settings-grid">
        <NumberField label="Xabar limiti" hint="0 yoki 3–100" value={draft.flood_limit} min={0} max={100} onChange={(value) => set("flood_limit", value)} />
        <NumberField label="Vaqt oynasi" hint="1–3600 sekund" value={draft.flood_window_secs} min={1} max={3600} onChange={(value) => set("flood_window_secs", value)} />
        <label className="field"><span>Limitdan keyin</span><select value={draft.flood_action} onChange={(event) => set("flood_action", event.target.value as ChatSettings["flood_action"])}><option value="delete">Delete</option><option value="warn">Warn</option><option value="mute">Mute</option><option value="ban">Ban</option></select></label>
      </div>
    </Card>
    <Card>
      <SectionHeader title="Warning siyosati" meta="Limitdan keyin avtomatik jazo" />
      <div className="settings-grid">
        <NumberField label="Warning limiti" hint="1–100" value={draft.warn_limit} min={1} max={100} onChange={(value) => set("warn_limit", value)} />
        <label className="field"><span>Avtomatik jazo</span><select value={draft.warn_action} onChange={(event) => set("warn_action", event.target.value as "mute" | "ban")}><option value="mute">Mute</option><option value="ban">Ban</option></select></label>
        <label className="field"><span>Mute davomiyligi</span><select value={draft.mute_duration_secs} onChange={(event) => set("mute_duration_secs", Number(event.target.value))}><option value={1800}>30 daqiqa</option><option value={3600}>1 soat</option><option value={21_600}>6 soat</option><option value={86_400}>1 kun</option><option value={604_800}>7 kun</option></select></label>
      </div>
    </Card>
    <Button className="full-width" disabled={mutation.isPending} onClick={() => mutation.mutate(draft)}>{mutation.isPending ? "Saqlanmoqda…" : "Sozlamalarni saqlash"}</Button>
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </>;
}

function ContentPanel() {
  const { api, chatId } = useApp();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [newTerm, setNewTerm] = useState("");
  const [deleteTerm, setDeleteTerm] = useState<BlockedTerm>();
  const [draft, setDraft] = useState<ChatSettings>();
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  const settingsQuery = useQuery({ queryKey: ["settings", chatId], queryFn: () => api.getSettings(chatId!), enabled: !!chatId });
  const blocklistQuery = useQuery({ queryKey: ["blocklist", chatId, search], queryFn: () => api.getBlocklist(chatId!, search), enabled: !!chatId });
  useEffect(() => { if (settingsQuery.data) setDraft(settingsQuery.data); }, [settingsQuery.data]);
  const save = useMutation({ mutationFn: (value: ChatSettings) => api.patchSettings(chatId!, { welcome_enabled: value.welcome_enabled, welcome_template: value.welcome_template, rules: value.rules }), onSuccess: (value) => { setDraft(value); void queryClient.invalidateQueries({ queryKey: ["settings", chatId] }); setToast({ message: "Kontent sozlamalari saqlandi", tone: "success" }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  const add = useMutation({ mutationFn: () => api.addBlockedTerm(chatId!, newTerm.trim()), onSuccess: () => { setNewTerm(""); void queryClient.invalidateQueries({ queryKey: ["blocklist", chatId] }); setToast({ message: "Ibora blocklistga qo‘shildi", tone: "success" }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  const remove = useMutation({ mutationFn: (term: BlockedTerm) => api.deleteBlockedTerm(chatId!, term.id), onSuccess: () => { setDeleteTerm(undefined); void queryClient.invalidateQueries({ queryKey: ["blocklist", chatId] }); setToast({ message: "Ibora olib tashlandi", tone: "success" }); }, onError: (error) => { setDeleteTerm(undefined); setToast({ message: errorMessage(error), tone: "error" }); } });
  const set = <K extends keyof ChatSettings>(key: K, value: ChatSettings[K]) => setDraft((current) => current ? { ...current, [key]: value } : current);

  if (!draft) return <StateView kind="loading" />;
  return <>
    <Card>
      <SectionHeader title="Blocklist" meta={`Unicode-aware · ${blocklistQuery.data?.items.length ?? 0} ibora`} />
      <div className="inline-form"><input aria-label="Yangi taqiqlangan ibora" maxLength={200} placeholder="Yangi ibora" value={newTerm} onChange={(event) => setNewTerm(event.target.value)} /><Button disabled={!newTerm.trim() || add.isPending} onClick={() => add.mutate()}><Plus /> Qo‘shish</Button></div>
      <SearchField aria-label="Blocklistdan qidirish" placeholder="Iborani qidiring" value={search} onChange={(event) => setSearch(event.target.value)} />
      {blocklistQuery.isLoading ? <StateView kind="loading" /> : !blocklistQuery.data?.items.length ? <StateView kind="empty" message="Blocklist bo‘sh" /> : <div className="term-list">{blocklistQuery.data.items.map((term) => <div className="term-row" key={term.id}><span className="list-icon lime"><Search /></span><div><strong>{term.term}</strong><small>{term.match_count} marta bloklangan · {relativeTime(term.last_matched_at)}</small></div><button className="remove-button" type="button" aria-label={`${term.term} iborasini o‘chirish`} onClick={() => setDeleteTerm(term)}><Trash2 /></button></div>)}</div>}
    </Card>
    <Card>
      <SectionHeader title="Guruh qoidalari" meta={`${draft.rules.length} / 3500`} />
      <label className="field"><span className="sr-only">Guruh qoidalari</span><textarea rows={7} maxLength={3500} value={draft.rules} onChange={(event) => set("rules", event.target.value)} /></label>
    </Card>
    <Card>
      <SectionHeader title="Welcome xabari" meta="Yangi a’zoga avtomatik yuboriladi" action={<Toggle label="Welcome xabari" checked={draft.welcome_enabled} onChange={(value) => set("welcome_enabled", value)} />} />
      <label className="field"><span className="sr-only">Welcome shabloni</span><textarea rows={5} maxLength={3500} disabled={!draft.welcome_enabled} value={draft.welcome_template} onChange={(event) => set("welcome_template", event.target.value)} /></label>
      <div className="chip-row">{["{first_name}", "{username}", "{user_id}", "{chat_title}"].map((placeholder) => <button type="button" key={placeholder} onClick={() => set("welcome_template", `${draft.welcome_template}${draft.welcome_template.endsWith(" ") ? "" : " "}${placeholder}`)}>{placeholder}</button>)}</div>
    </Card>
    <Button className="full-width" disabled={save.isPending} onClick={() => save.mutate(draft)}>{save.isPending ? "Saqlanmoqda…" : "Kontentni saqlash"}</Button>
    <ConfirmDialog open={!!deleteTerm} title="Blocklistdan o‘chirish" message={`“${deleteTerm?.term ?? ""}” iborasini blocklistdan olib tashlashni tasdiqlaysizmi?`} confirmLabel="O‘chirish" danger pending={remove.isPending} onClose={() => setDeleteTerm(undefined)} onConfirm={() => deleteTerm && remove.mutate(deleteTerm)} />
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </>;
}

function ModulesPanel() {
  const { api, chatId } = useApp();
  const queryClient = useQueryClient();
  const modulesQuery = useQuery({ queryKey: ["modules", chatId], queryFn: () => api.getModules(chatId!), enabled: !!chatId });
  const healthQuery = useQuery({ queryKey: ["health", chatId], queryFn: () => api.getHealth(chatId!), enabled: !!chatId });
  const overviewQuery = useQuery({ queryKey: ["overview", chatId], queryFn: () => api.getOverview(chatId!), enabled: !!chatId });
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  const mutation = useMutation({ mutationFn: ({ key, enabled }: { key: string; enabled: boolean }) => api.patchModule(chatId!, key, { enabled }), onSuccess: () => { void queryClient.invalidateQueries({ queryKey: ["modules", chatId] }); void queryClient.invalidateQueries({ queryKey: ["overview", chatId] }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  const grouped = useMemo(() => {
    const rows = modulesQuery.data?.items ?? [];
    return { required: rows.filter((item) => requiredModules.has(item.module_key)), available: rows.filter((item) => !requiredModules.has(item.module_key) && !unavailableModules.has(item.module_key)), unavailable: rows.filter((item) => unavailableModules.has(item.module_key)) };
  }, [modulesQuery.data]);
  if (modulesQuery.isLoading || healthQuery.isLoading) return <StateView kind="loading" message="Modullar va Telegram huquqlari tekshirilmoqda" />;
  if (modulesQuery.error || healthQuery.error) return <StateView kind="error" message={errorMessage(modulesQuery.error || healthQuery.error)} />;
  const health = healthQuery.data!;
  return <>
    <Card className="score-card compact-score"><div className="score-copy"><span className="eyebrow">PROTECTION SCORE</span><strong className="score-value">{overviewQuery.data?.protection_score ?? "—"}%</strong><small>{modulesQuery.data?.enabled} / {modulesQuery.data?.total} modul yoqilgan</small></div><ShieldCheck className="hero-icon" /></Card>
    <Card>
      <SectionHeader title="Runtime health" action={<Badge tone={health.bot_status === "active" ? "success" : "danger"}>{health.bot_status}</Badge>} />
      <div className="health-grid">
        <HealthItem icon={<ShieldCheck />} label="Bot mavjud" ok={health.telegram.bot_present} />
        <HealthItem icon={<UsersRound />} label="Bot admin" ok={health.telegram.bot_admin} />
        <HealthItem icon={<Database />} label="PostgreSQL" ok={health.database_ready} />
        <HealthItem icon={<ListChecks />} label="Delete" ok={health.telegram.can_delete_messages} />
        <HealthItem icon={<WandSparkles />} label="Restrict" ok={health.telegram.can_restrict_members} />
        <HealthItem icon={<ShieldCheck />} label="Ban" ok={health.telegram.can_ban_members} />
      </div>
      {health.bot_status !== "active" && <div className="remediation"><strong>Himoya cheklangan</strong><p>Botga Delete messages va Ban users huquqlarini bering, so‘ng holatni yangilang.</p><Button variant="secondary" onClick={() => void healthQuery.refetch()}>Qayta tekshirish</Button></div>}
    </Card>
    <ModuleGroup title="Asosiy modullar" items={grouped.required} mutation={mutation} />
    <ModuleGroup title="Sozlanadigan modullar" items={grouped.available} mutation={mutation} />
    <ModuleGroup title="Keyingi relizlar" items={grouped.unavailable} mutation={mutation} />
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </>;
}

function NumberField({ label, hint, value, min, max, onChange }: { label: string; hint: string; value: number; min: number; max: number; onChange(value: number): void }) {
  return <label className="field"><span>{label}<small>{hint}</small></span><input type="number" min={min} max={max} value={value} onChange={(event) => onChange(Number(event.target.value))} /></label>;
}

function HealthItem({ icon, label, ok }: { icon: React.ReactNode; label: string; ok: boolean }) {
  return <div className="health-item"><span>{icon}</span><strong>{label}</strong><StatusDot ok={ok} /></div>;
}

function ModuleGroup({ title, items, mutation }: { title: string; items: ProtectionModule[]; mutation: ReturnType<typeof useMutation<ProtectionModule, Error, { key: string; enabled: boolean }>> }) {
  return <Card><SectionHeader title={title} meta={`${items.length} modul`} /><div className="module-list">{items.map((item) => { const required = requiredModules.has(item.module_key); const unavailable = unavailableModules.has(item.module_key); return <div className="module-row" key={item.module_key}><span className={`list-icon ${item.enabled && item.healthy ? "lime" : "muted"}`}><ShieldCheck /></span><div><strong>{item.title}</strong><small>{item.module_key} · {item.configured ? "sozlangan" : "sozlanmagan"}</small></div>{unavailable ? <Badge>TEZ ORADA</Badge> : required ? <Badge tone="lime">MAJBURIY</Badge> : <Toggle label={`${item.title} modulini yoqish`} checked={item.enabled} disabled={mutation.isPending} onChange={(enabled) => mutation.mutate({ key: item.module_key, enabled })} />}</div>; })}</div></Card>;
}
