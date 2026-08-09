import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpenText, Bot, Check, ChevronDown, CircleUserRound, Database, KeyRound, ShieldCheck, UsersRound } from "lucide-react";
import { errorMessage } from "../api";
import { useApp } from "../app-context";
import { Badge, Card, PageHeader, SectionHeader, StateView, StatusDot } from "../components";
import { initials, relativeTime } from "../format";

const commands = [
  ["Umumiy", [["/start", "Botni ishga tushirish"], ["/help", "Buyruqlar ro‘yxati"], ["/settings", "Guruh sozlamalari"], ["/rules", "Guruh qoidalari"]]],
  ["Moderatsiya", [["/warn [sabab]", "A’zoni ogohlantirish"], ["/unwarn", "Warninglarni tozalash"], ["/warnings", "Warning soni"], ["/ban", "A’zoni ban qilish"], ["/unban <user_id>", "Bandan chiqarish"], ["/mute [30m|2h|1d]", "Vaqtincha cheklash"], ["/unmute", "Mute’ni olib tashlash"]]],
  ["Himoya siyosati", [["/setflood <limit> [s] [action]", "Anti-flood sozlamasi"], ["/setwarnlimit <1..100>", "Warning limiti"], ["/welcome on|off", "Welcome holati"], ["/setwelcome <matn>", "Welcome shabloni"]]],
  ["Kontent", [["/setrules <matn>", "Qoidalarni o‘rnatish"], ["/addblock <ibora>", "Blocklistga qo‘shish"], ["/rmblock <ibora>", "Blocklistdan o‘chirish"], ["/blocklist", "Blocklist ro‘yxati"]]],
] as const;

export function MoreScreen() {
  const { api, chatId, me, chats, setChatId, demo } = useApp();
  const [commandsOpen, setCommandsOpen] = useState(false);
  const health = useQuery({ queryKey: ["health", chatId], queryFn: () => api.getHealth(chatId!), enabled: !!chatId });
  const name = [me?.user.first_name, me?.user.last_name].filter(Boolean).join(" ") || "Telegram admin";
  return <div className="page-stack">
    <PageHeader title="Admin" subtitle="Telegram Mini App sessiyasi" />
    <Card className="admin-card"><span className="avatar avatar-large">{initials(name)}</span><div><span className="eyebrow">GET /API/ME</span><strong>{name}</strong><small>@{me?.user.username ?? "username yo‘q"} · ID {me?.user.id}</small></div><Badge tone="success">{demo ? "DEMO" : "AUTH"}</Badge></Card>
    <Card>
      <SectionHeader title="Boshqariladigan guruhlar" meta={`${chats.length} guruh`} />
      <div className="group-list">{chats.map((chat) => <button type="button" key={chat.chat_id} onClick={() => setChatId(chat.chat_id)}><span className="avatar">{initials(chat.title)}</span><span><strong>{chat.title}</strong><small>{chat.chat_type}{chat.username || chat.chat_username ? ` · @${chat.username ?? chat.chat_username}` : ""}</small></span><span><Badge tone={chat.chat_id === chatId ? "lime" : "neutral"}>{chat.chat_id === chatId ? "TANLANGAN" : "ADMIN"}</Badge><small>{relativeTime(chat.last_update_at)}</small></span></button>)}</div>
    </Card>
    <Card>
      <SectionHeader title="Botni sozlash" meta="3 asosiy qadam" />
      <div className="setup-list"><Setup icon={<Bot />} title="Botni guruhga qo‘shing" text="CheklaBot’ni guruhga taklif qiling" /><Setup icon={<ShieldCheck />} title="Admin huquqlarini bering" text="Delete messages · Ban users" /><Setup icon={<KeyRound />} title="Mini App’ni oching" text="Signed Telegram initData orqali" /></div>
    </Card>
    <Card>
      <button className="accordion-trigger" type="button" aria-expanded={commandsOpen} onClick={() => setCommandsOpen((value) => !value)}><span className="list-icon lime"><BookOpenText /></span><span><strong>Telegram buyruqlari</strong><small>19 ta command · backend bilan mos</small></span><ChevronDown className={commandsOpen ? "rotated" : ""} /></button>
      {commandsOpen && <div className="command-guide">{commands.map(([group, rows]) => <section key={group}><h3>{group}</h3>{rows.map(([command, text]) => <div key={command}><code>{command}</code><span>{text}</span></div>)}</section>)}</div>}
    </Card>
    <Card>
      <SectionHeader title="Tizim holati" action={health.data && <Badge tone={health.data.bot_status === "active" ? "success" : "danger"}>{health.data.bot_status}</Badge>} />
      {health.isLoading ? <StateView kind="loading" /> : health.error ? <StateView kind="error" message={errorMessage(health.error)} /> : <div className="compact-list"><SystemRow icon={<Bot />} label="Telegram bot" ok={!!health.data?.telegram.bot_present} /><SystemRow icon={<UsersRound />} label="Admin huquqlari" ok={!!health.data?.telegram.bot_admin} /><SystemRow icon={<Database />} label="PostgreSQL" ok={!!health.data?.database_ready} /><SystemRow icon={<CircleUserRound />} label="Admin sessiyasi" ok /></div>}
    </Card>
  </div>;
}

function Setup({ icon, title, text }: { icon: React.ReactNode; title: string; text: string }) {
  return <div><span className="setup-check"><Check /></span><span className="list-icon muted">{icon}</span><p><strong>{title}</strong><small>{text}</small></p></div>;
}

function SystemRow({ icon, label, ok }: { icon: React.ReactNode; label: string; ok: boolean }) {
  return <div className="compact-row"><span className="list-icon muted">{icon}</span><div><strong>{label}</strong><small>{ok ? "Ishlayapti" : "Tekshiruv kerak"}</small></div><StatusDot ok={ok} /></div>;
}
