import { useDeferredValue, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { Ban, CheckCircle2, Clock3, RotateCcw, ShieldAlert, UserRound, Volume2, VolumeX } from "lucide-react";
import { errorMessage } from "../api";
import { useApp } from "../app-context";
import { Badge, Button, Card, ConfirmDialog, PageHeader, SearchField, SectionHeader, Segmented, StateView, Toast } from "../components";
import { displayName, initials, relativeTime } from "../format";
import { haptic } from "../telegram";
import type { ChatMember } from "../types";

type Action = "warn" | "mute" | "ban";
type PendingAction = Action | "unwarn" | "unmute" | "unban";

export function ModerationScreen() {
  const { api, chatId } = useApp();
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search.trim());
  const [selected, setSelected] = useState<ChatMember>();
  const [action, setAction] = useState<Action>("warn");
  const [reason, setReason] = useState("Takroriy reklama xabari");
  const [duration, setDuration] = useState(3600);
  const [pendingAction, setPendingAction] = useState<PendingAction>();
  const [toast, setToast] = useState<{ message: string; tone: "success" | "error" }>();
  const previousChatId = useRef(chatId);
  const requestedMemberValue = searchParams.get("member");
  const requestedMemberId = requestedMemberValue && /^\d+$/.test(requestedMemberValue) ? Number(requestedMemberValue) : undefined;

  const members = useQuery({ queryKey: ["members", chatId, deferredSearch], queryFn: () => api.getMembers(chatId!, deferredSearch), enabled: !!chatId });
  const requestedMember = useQuery({ queryKey: ["member", chatId, requestedMemberId], queryFn: () => api.getMember(chatId!, requestedMemberId!), enabled: !!chatId && !!requestedMemberId });
  useEffect(() => {
    if (requestedMember.data) setSelected(requestedMember.data);
  }, [requestedMember.data]);
  useEffect(() => {
    if (previousChatId.current !== chatId) {
      setSelected(undefined);
      setSearch("");
      setPendingAction(undefined);
      setSearchParams({}, { replace: true });
    }
    previousChatId.current = chatId;
  }, [chatId, setSearchParams]);
  const warnings = useQuery({ queryKey: ["warnings", chatId, selected?.user_id], queryFn: () => api.getWarnings(chatId!, selected!.user_id), enabled: !!chatId && !!selected });
  const history = useQuery({ queryKey: ["member-audit", chatId, selected?.user_id], queryFn: () => api.getAudit(chatId!, { target_user_id: selected!.user_id, limit: 5 }), enabled: !!chatId && !!selected });

  const mutation = useMutation({
    mutationFn: async (nextAction: PendingAction) => {
      if (!selected) throw new Error("Foydalanuvchini tanlang");
      return api.moderate(chatId!, nextAction, { target_user_id: selected.user_id, reason: reason.trim() || undefined, duration_secs: nextAction === "mute" ? duration : undefined });
    },
    onSuccess: (result) => {
      haptic("success");
      const automatic = result.automatic_sanction ? ` Avtomatik jazo: ${result.automatic_sanction}.` : "";
      setToast({ message: `Amal muvaffaqiyatli bajarildi.${automatic}`, tone: "success" });
      setPendingAction(undefined);
      void queryClient.invalidateQueries({ queryKey: ["warnings", chatId, selected?.user_id] });
      void queryClient.invalidateQueries({ queryKey: ["overview", chatId] });
      void queryClient.invalidateQueries({ queryKey: ["member-audit", chatId, selected?.user_id] });
    },
    onError: (error) => {
      haptic("error");
      setToast({ message: errorMessage(error), tone: "error" });
      setPendingAction(undefined);
    },
  });

  const requestAction = (nextAction: PendingAction) => {
    if (!selected) return;
    if (["ban", "unwarn", "unmute", "unban"].includes(nextAction)) setPendingAction(nextAction);
    else mutation.mutate(nextAction);
  };
  const selectMember = (member: ChatMember) => {
    setSelected(member);
    setSearch("");
    setSearchParams({ member: String(member.user_id) }, { replace: true });
    haptic("success");
  };

  return <div className="page-stack">
    <PageHeader title="Moderatsiya" subtitle="A’zolar indeksidan tanlang" />
    <div className="member-search-wrap">
      <SearchField type="search" aria-label="A’zolarni qidirish" placeholder="Username, ism-familiya yoki Telegram ID" value={search} onChange={(event) => setSearch(event.target.value)} />
      {search.trim() && <div className="search-results" role="listbox" aria-label="Qidiruv natijalari" aria-busy={members.isFetching}>
        {members.isLoading || (members.isFetching && !members.data) ? <StateView kind="loading" message="A’zolar qidirilmoqda" />
          : members.error ? <StateView kind="error" message={errorMessage(members.error)} />
          : !members.data?.items.length ? <StateView kind="empty" message="Username, ism-familiya yoki ID bo‘yicha a’zo topilmadi" />
          : members.data.items.slice(0, 20).map((member) => <button type="button" role="option" aria-selected={selected?.user_id === member.user_id} key={member.user_id} onClick={() => selectMember(member)}><Avatar member={member} /><span><strong>{displayName(member)}</strong><small>@{member.username ?? "username yo‘q"} · ID {member.user_id}</small></span><Badge>{member.status}</Badge></button>)}
      </div>}
    </div>

    {!selected && !search.trim() && requestedMember.isLoading && <StateView kind="loading" message="A’zo yuklanmoqda" />}
    {!selected && !search.trim() && !requestedMember.isLoading && <StateView kind="empty" message="Username, yozilgan ism yoki Telegram ID orqali foydalanuvchini toping" />}
    {selected && !search.trim() && <>
      <Card className="member-card">
        <div className="member-identity"><Avatar member={selected} /><div><strong>{displayName(selected)}</strong><small>@{selected.username ?? "username yo‘q"} · ID {selected.user_id}</small></div><Badge tone={selected.status === "active" || selected.status === "member" ? "success" : "lime"}>{selected.status}</Badge></div>
        <div className="warning-progress"><div><span>Warning holati</span><strong>{warnings.data?.count ?? 0} / {warnings.data?.limit ?? "—"}</strong></div><progress max={warnings.data?.limit ?? 3} value={warnings.data?.count ?? 0} /></div>
      </Card>

      <Segmented<Action> label="Moderatsiya amali" value={action} onChange={setAction} options={[{ value: "warn", label: "Warn" }, { value: "mute", label: "Mute" }, { value: "ban", label: "Ban" }]} />
      <div className="action-explainer"><span className={`action-symbol ${action}`} aria-hidden="true">{action === "warn" ? <ShieldAlert /> : action === "mute" ? <VolumeX /> : <Ban />}</span><div><strong>{action === "warn" ? "Ogohlantirish" : action === "mute" ? "Vaqtincha cheklash" : "Guruhdan chiqarish"}</strong><p>{action === "warn" ? "Limitga yetganda avtomatik jazo qo‘llanadi" : action === "mute" ? "30 sekunddan 365 kungacha" : "Foydalanuvchi guruhga yoza olmaydi"}</p></div></div>

      <Card>
        <label className="field"><span>Sabab <small>{reason.length} / 1000</small></span><textarea maxLength={1000} rows={3} value={reason} onChange={(event) => setReason(event.target.value)} /></label>
        <div className="chip-row">{["Reklama", "Spam", "Haqorat", "Boshqa"].map((value) => <button type="button" key={value} onClick={() => setReason(value)}>{value}</button>)}</div>
        {action === "mute" && <label className="field"><span>Davomiylik</span><select value={duration} onChange={(event) => setDuration(Number(event.target.value))}><option value={1800}>30 daqiqa</option><option value={3600}>1 soat</option><option value={21_600}>6 soat</option><option value={86_400}>1 kun</option><option value={604_800}>7 kun</option></select></label>}
      </Card>
      <Button className="full-width" disabled={mutation.isPending} onClick={() => requestAction(action)}>{mutation.isPending ? "Bajarilmoqda…" : action === "warn" ? "Ogohlantirish yuborish" : action === "mute" ? "Mute qilish" : "Ban qilish"}</Button>

      <Card>
        <SectionHeader title="Tiklash amallari" meta="Barcha amallar auditga yoziladi" />
        <div className="recovery-grid">
          <Button variant="secondary" onClick={() => requestAction("unwarn")}><RotateCcw /> Warninglarni tozalash</Button>
          <Button variant="secondary" onClick={() => requestAction("unmute")}><Volume2 /> Mute’ni olish</Button>
          <Button variant="secondary" onClick={() => requestAction("unban")}><UserRound /> Bandan chiqarish</Button>
        </div>
      </Card>

      <Card>
        <SectionHeader title="A’zo tarixi" meta="Audit jurnalidan" />
        {!history.data?.items.length ? <StateView kind="empty" message="Bu a’zo bo‘yicha tarix topilmadi" /> : <div className="compact-list">{history.data.items.map((item) => <div className="compact-row" key={item.id}><span className="list-icon muted"><Clock3 /></span><div><strong>{item.action}</strong><small>{item.reason ?? "Sabab ko‘rsatilmagan"}</small></div><time>{relativeTime(item.created_at)}</time></div>)}</div>}
      </Card>
    </>}

    <ConfirmDialog open={!!pendingAction} title={confirmTitle(pendingAction)} message={`${selected ? displayName(selected) : "Foydalanuvchi"} uchun bu amalni bajarishni tasdiqlaysizmi? Amal audit jurnaliga yoziladi.`} confirmLabel="Tasdiqlash" danger={pendingAction === "ban"} pending={mutation.isPending} onClose={() => setPendingAction(undefined)} onConfirm={() => pendingAction && mutation.mutate(pendingAction)} />
    <Toast message={toast?.message} tone={toast?.tone} onClose={() => setToast(undefined)} />
  </div>;
}

function Avatar({ member }: { member: ChatMember }) {
  return <span className="avatar" aria-hidden="true">{initials(displayName(member))}</span>;
}

function confirmTitle(action?: PendingAction): string {
  return ({ ban: "Ban qilish", unwarn: "Warninglarni tozalash", unmute: "Mute’ni olib tashlash", unban: "Bandan chiqarish", warn: "Ogohlantirish", mute: "Mute qilish" } as Record<string, string>)[action ?? ""] ?? "Amalni tasdiqlash";
}
