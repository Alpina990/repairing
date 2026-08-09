import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Ban, Clock3, MessageSquareWarning, ShieldCheck, Sparkles, Zap } from "lucide-react";
import { useApp } from "../app-context";
import { Badge, Card, PageHeader, SectionHeader, StateView, StatusDot } from "../components";
import { errorMessage } from "../api";
import { formatDuration, relativeTime } from "../format";

const actionLabels: Record<string, string> = {
  warn: "Ogohlantirish berildi",
  mute: "Vaqtincha mute qilindi",
  ban: "Guruhdan chiqarildi",
  delete: "Xabar o‘chirildi",
  blocklist_match: "Blocklist mosligi topildi",
  anti_flood: "Flood avtomatik to‘xtatildi",
  unmute: "Mute olib tashlandi",
  unban: "Bandan chiqarildi",
};

export function OverviewScreen() {
  const { api, chatId, chat } = useApp();
  const overview = useQuery({ queryKey: ["overview", chatId], queryFn: () => api.getOverview(chatId!), enabled: !!chatId });
  if (overview.isLoading) return <StateView kind="loading" message="Himoya holati hisoblanmoqda" />;
  if (overview.error) return <StateView kind="error" message={errorMessage(overview.error)} />;
  const data = overview.data!;
  const scoreMissing = data.protection_score_breakdown.filter((item) => item.earned < item.weight).slice(0, 3);

  return <div className="page-stack">
    <PageHeader title="CheklaBot" subtitle={`${chat?.title ?? "Guruh"} · real vaqt himoyasi`} />
    <Card className="score-card">
      <div className="score-copy">
        <span className="eyebrow">AUTO-PROTECT</span>
        <strong className="score-value">{data.protection_score}%</strong>
        <span className="score-state"><StatusDot ok={data.bot_status === "active"} />{data.bot_status === "active" ? "Himoya faol" : "Himoya cheklangan"}</span>
        <small>{data.enabled_modules} / {data.total_modules} modul yoqilgan</small>
      </div>
      <div className="score-ring" style={{ "--score": `${data.protection_score * 3.6}deg` } as React.CSSProperties} aria-label={`Himoya darajasi ${data.protection_score} foiz`}><ShieldCheck /></div>
    </Card>

    <div className="metric-grid">
      <Metric icon={<Ban />} value={data.auto_blocks_today} label="Auto-block" />
      <Metric icon={<MessageSquareWarning />} value={data.warnings_today} label="Warning" />
      <Metric icon={<Clock3 />} value={formatDuration(data.average_response_seconds)} label="Javob vaqti" />
      <Metric icon={<AlertTriangle />} value={data.open_incidents} label="Ochiq incident" />
    </div>

    {scoreMissing.length > 0 && <Card>
      <SectionHeader title="Himoyani kuchaytirish" meta="Score breakdown asosida" />
      <div className="compact-list">
        {scoreMissing.map((item) => <div className="compact-row" key={item.module_key}><span className="list-icon muted"><Sparkles /></span><div><strong>{moduleTitle(item.module_key)}</strong><small>+{item.weight - item.earned}% imkoniyat</small></div><Badge>{item.earned}/{item.weight}</Badge></div>)}
      </div>
    </Card>}

    <Card>
      <SectionHeader title="So‘nggi faollik" meta={`Bugun · ${data.recent_activity.length} ko‘rsatilmoqda`} />
      {data.recent_activity.length === 0 ? <StateView kind="empty" message="Moderatsiya amallari hali yo‘q" /> : <div className="timeline">
        {data.recent_activity.slice(0, 6).map((item) => <article className="timeline-item" key={item.id}>
          <span className={`list-icon ${item.status === "failed" ? "danger" : item.source === "auto" ? "lime" : "muted"}`}>{item.source === "auto" ? <Zap /> : <ShieldCheck />}</span>
          <div><div className="row-title"><strong>{actionLabels[item.action] ?? item.action}</strong><Badge tone={item.source === "auto" ? "lime" : "neutral"}>{item.source}</Badge></div><p>@{item.target_username ?? item.target_id}{item.reason ? ` · ${item.reason}` : ""}</p></div>
          <time>{relativeTime(item.created_at)}</time>
        </article>)}
      </div>}
    </Card>
  </div>;
}

function Metric({ icon, value, label }: { icon: React.ReactNode; value: React.ReactNode; label: string }) {
  return <Card className="metric-card"><span>{icon}</span><strong>{value}</strong><small>{label}</small></Card>;
}

function moduleTitle(key: string): string {
  return ({ captcha: "CAPTCHA", anti_raid: "Anti-raid", link_filter: "Link filtri", reports: "Shikoyatlar", blocklist: "Blocklist", member_index: "A’zolar indeksi" } as Record<string, string>)[key] ?? key.replaceAll("_", " ");
}
