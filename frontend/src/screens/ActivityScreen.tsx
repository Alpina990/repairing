import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, CheckCircle2, Download, Filter, Radio, ShieldCheck, Zap } from "lucide-react";
import { errorMessage } from "../api";
import { useApp } from "../app-context";
import { Badge, Button, Card, ConfirmDialog, PageHeader, SearchField, SectionHeader, Segmented, StateView, Toast } from "../components";
import { relativeTime } from "../format";
import type { Incident } from "../types";

type ActivityTab = "audit" | "incidents";

export function ActivityScreen() {
  const [tab, setTab] = useState<ActivityTab>("audit");
  return <div className="page-stack"><PageHeader title="Faollik" subtitle="Audit va incidentlar markazi" /><Segmented<ActivityTab> value={tab} onChange={setTab} label="Faollik bo‘limi" options={[{ value: "audit", label: "Audit" }, { value: "incidents", label: "Incidentlar" }]} />{tab === "audit" ? <AuditPanel /> : <IncidentsPanel />}</div>;
}

function AuditPanel() {
  const { api, chatId } = useApp();
  const [action, setAction] = useState("");
  const [source, setSource] = useState("");
  const [search, setSearch] = useState("");
  const [showFilters, setShowFilters] = useState(false);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  const filters = { action: action || undefined, source: (source || undefined) as "auto" | "admin" | undefined, q: search || undefined, from: from ? new Date(from).toISOString() : undefined, to: to ? new Date(to).toISOString() : undefined, limit: 100 };
  const audit = useQuery({ queryKey: ["audit", chatId, filters], queryFn: () => api.getAudit(chatId!, filters), enabled: !!chatId });
  const exportMutation = useMutation({ mutationFn: () => api.exportAudit(chatId!, filters), onSuccess: (blob) => { const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `cheklabot-audit-${chatId}.csv`; anchor.click(); URL.revokeObjectURL(url); setToast({ message: "Audit CSV eksport qilindi", tone: "success" }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  return <>
    <div className="toolbar"><SearchField aria-label="Auditdan qidirish" placeholder="A’zo, sabab yoki amal" value={search} onChange={(event) => setSearch(event.target.value)} /><Button variant="secondary" aria-expanded={showFilters} onClick={() => setShowFilters((value) => !value)}><Filter /> Filtr</Button></div>
    {showFilters && <Card className="filter-panel"><div className="settings-grid"><label className="field"><span>Amal</span><select value={action} onChange={(event) => setAction(event.target.value)}><option value="">Barchasi</option><option value="warn">Warn</option><option value="mute">Mute</option><option value="ban">Ban</option><option value="unwarn">Unwarn</option><option value="unmute">Unmute</option><option value="unban">Unban</option><option value="delete">Delete</option><option value="blocklist_match">Blocklist</option><option value="anti_flood">Anti-flood</option><option value="settings_update">Settings</option></select></label><label className="field"><span>Manba</span><select value={source} onChange={(event) => setSource(event.target.value)}><option value="">Barchasi</option><option value="auto">Avtomatik</option><option value="admin">Admin</option></select></label><label className="field"><span>Boshlanish</span><input type="datetime-local" value={from} onChange={(event) => setFrom(event.target.value)} /></label><label className="field"><span>Tugash</span><input type="datetime-local" value={to} onChange={(event) => setTo(event.target.value)} /></label></div></Card>}
    <Card>
      <SectionHeader title="Moderatsiya jurnali" meta={`${audit.data?.items.length ?? 0} yozuv`} action={<Button variant="ghost" disabled={exportMutation.isPending} onClick={() => exportMutation.mutate()}><Download /> CSV</Button>} />
      {audit.isLoading ? <StateView kind="loading" /> : audit.error ? <StateView kind="error" message={errorMessage(audit.error)} /> : !audit.data?.items.length ? <StateView kind="empty" message="Tanlangan filtrlarga mos yozuv yo‘q" /> : <div className="timeline">{audit.data.items.map((item) => <article className="timeline-item" key={item.id}><span className={`list-icon ${item.status === "failed" ? "danger" : item.source === "auto" ? "lime" : "muted"}`}>{item.source === "auto" ? <Zap /> : <ShieldCheck />}</span><div><div className="row-title"><strong>{item.action.replaceAll("_", " ")}</strong><Badge tone={item.status === "failed" ? "danger" : item.source === "auto" ? "lime" : "neutral"}>{item.source} · {item.status}</Badge></div><p>@{item.target_username ?? item.target_id}{item.reason ? ` · ${item.reason}` : ""}</p></div><time>{relativeTime(item.created_at)}</time></article>)}</div>}
    </Card>
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </>;
}

function IncidentsPanel() {
  const { api, chatId } = useApp();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState("");
  const [selected, setSelected] = useState<Incident>();
  const [resolveTarget, setResolveTarget] = useState<Incident>();
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  const incidents = useQuery({ queryKey: ["incidents", chatId, status], queryFn: () => api.getIncidents(chatId!, status || undefined), enabled: !!chatId });
  const mutation = useMutation({ mutationFn: ({ incident, next }: { incident: Incident; next: "acknowledged" | "resolved" }) => api.patchIncident(chatId!, incident.id, next), onSuccess: (item) => { setSelected(item); setResolveTarget(undefined); void queryClient.invalidateQueries({ queryKey: ["incidents", chatId] }); void queryClient.invalidateQueries({ queryKey: ["overview", chatId] }); setToast({ message: "Incident holati yangilandi", tone: "success" }); }, onError: (error) => setToast({ message: errorMessage(error), tone: "error" }) });
  return <>
    <Segmented value={status} onChange={setStatus} label="Incident holati" options={[{ value: "", label: "Barchasi" }, { value: "detected", label: "Detected" }, { value: "acknowledged", label: "Ack" }, { value: "resolved", label: "Resolved" }]} />
    <Card>
      <SectionHeader title="Ustuvor navbat" meta={`${incidents.data?.items.filter((item) => item.status !== "resolved").length ?? 0} ochiq incident`} />
      {incidents.isLoading ? <StateView kind="loading" /> : incidents.error ? <StateView kind="error" message={errorMessage(incidents.error)} /> : !incidents.data?.items.length ? <StateView kind="empty" message="Incident topilmadi" /> : <div className="incident-list">{incidents.data.items.map((item) => <button className={`incident-row${selected?.id === item.id ? " selected" : ""}`} type="button" key={item.id} onClick={() => setSelected(item)}><span className={`list-icon ${item.status === "detected" ? "danger" : item.status === "acknowledged" ? "muted" : "lime"}`}>{item.status === "resolved" ? <CheckCircle2 /> : item.status === "detected" ? <AlertTriangle /> : <Radio />}</span><span><strong>{item.incident_type.replaceAll("_", " ")}</strong><small>#{item.id} · {metadataText(item)}</small></span><span><Badge tone={item.status === "detected" ? "danger" : item.status === "resolved" ? "success" : "neutral"}>{item.status}</Badge><time>{relativeTime(item.detected_at)}</time></span></button>)}</div>}
    </Card>
    {selected && <Card className="incident-detail"><SectionHeader title={`${selected.incident_type.replaceAll("_", " ")} · #${selected.id}`} action={<Badge>{selected.severity}</Badge>} /><dl><div><dt>Aniqlandi</dt><dd>{new Date(selected.detected_at).toLocaleString("uz-UZ")}</dd></div><div><dt>Qabul qilindi</dt><dd>{selected.acknowledged_at ? new Date(selected.acknowledged_at).toLocaleString("uz-UZ") : "—"}</dd></div><div><dt>Hal qilindi</dt><dd>{selected.resolved_at ? new Date(selected.resolved_at).toLocaleString("uz-UZ") : "—"}</dd></div></dl><div className="dialog-actions">{selected.status === "detected" && <Button onClick={() => mutation.mutate({ incident: selected, next: "acknowledged" })}>Qabul qilish</Button>}{selected.status !== "resolved" && <Button variant="secondary" onClick={() => setResolveTarget(selected)}>Hal qilish</Button>}</div></Card>}
    <ConfirmDialog open={!!resolveTarget} title="Incidentni hal qilish" message={`#${resolveTarget?.id ?? ""} incidentini resolved holatiga o‘tkazishni tasdiqlaysizmi?`} confirmLabel="Hal qilindi" pending={mutation.isPending} onClose={() => setResolveTarget(undefined)} onConfirm={() => resolveTarget && mutation.mutate({ incident: resolveTarget, next: "resolved" })} />
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </>;
}

function metadataText(item: Incident): string {
  const values = Object.entries(item.metadata).slice(0, 2).map(([key, value]) => `${key}: ${String(value)}`);
  return values.join(" · ") || "metadata yo‘q";
}
