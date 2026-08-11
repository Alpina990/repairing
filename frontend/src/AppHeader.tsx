import { useQuery } from "@tanstack/react-query";
import { Check, ChevronDown, CircleUserRound, Search, X } from "lucide-react";
import { useDeferredValue, useEffect, useRef, useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { useApp } from "./app-context";
import { displayName, initials } from "./format";
import { haptic } from "./telegram";
import type { ChatMember } from "./types";

export function AppHeader() {
  const { chat, chats, chatId, setChatId } = useApp();
  const [groupsOpen, setGroupsOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setGroupsOpen(false);
        setSearchOpen(false);
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);

  return <>
    <header className="app-header">
      <div className="brand-lockup">
        <span className="eyebrow">CHEKLABOT</span>
        <button
          className="chat-picker-trigger"
          type="button"
          aria-label="Guruhni almashtirish"
          aria-haspopup="listbox"
          aria-expanded={groupsOpen}
          onClick={() => { haptic("tap"); setGroupsOpen((value) => !value); }}
        >
          <span>{chat?.title ?? "Guruh tanlanmagan"}</span>
          <ChevronDown className={groupsOpen ? "rotated" : ""} size={17} aria-hidden="true" />
        </button>
      </div>
      <div className="header-actions">
        <button className="icon-button" type="button" aria-label="A’zolarni qidirish" onClick={() => { haptic("tap"); setSearchOpen(true); }}><Search size={19} /></button>
        <NavLink className="icon-button" to="/more" aria-label="Admin profili"><CircleUserRound size={19} /></NavLink>
      </div>
      {groupsOpen && <>
        <button className="header-menu-backdrop" type="button" aria-label="Guruhlar ro‘yxatini yopish" onClick={() => setGroupsOpen(false)} />
        <div className="header-group-menu" role="listbox" aria-label="Boshqariladigan guruhlar">
          <strong>Guruhni tanlang</strong>
          {chats.map((item) => <button
            type="button"
            role="option"
            aria-selected={item.chat_id === chatId}
            key={item.chat_id}
            onClick={() => {
              setChatId(item.chat_id);
              setGroupsOpen(false);
              haptic("success");
            }}
          >
            <span className="avatar">{initials(item.title)}</span>
            <span><strong>{item.title}</strong><small>{item.chat_type}{item.username || item.chat_username ? ` · @${item.username ?? item.chat_username}` : ""}</small></span>
            {item.chat_id === chatId && <Check aria-label="Tanlangan" />}
          </button>)}
        </div>
      </>}
    </header>
    <GlobalMemberSearch open={searchOpen} onClose={() => setSearchOpen(false)} />
  </>;
}

function GlobalMemberSearch({ open, onClose }: { open: boolean; onClose(): void }) {
  const { api, chatId, chat } = useApp();
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search.trim());
  const members = useQuery({
    queryKey: ["global-member-search", chatId, deferredSearch],
    queryFn: () => api.getMembers(chatId!, deferredSearch),
    enabled: open && !!chatId,
  });

  useEffect(() => {
    if (open) {
      setSearch("");
      window.setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  if (!open) return null;
  const select = (member: ChatMember) => {
    onClose();
    haptic("success");
    navigate(`/moderation?member=${member.user_id}`);
  };

  return <div className="dialog-backdrop search-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="search-dialog" role="dialog" aria-modal="true" aria-labelledby="global-search-title">
      <div className="search-dialog-header"><div><span className="eyebrow">{chat?.title ?? "Guruh"}</span><h2 id="global-search-title">A’zolarni qidirish</h2></div><button type="button" aria-label="Yopish" onClick={onClose}><X /></button></div>
      <label className="search-field"><Search size={18} aria-hidden="true" /><span className="sr-only">A’zolarni qidirish</span><input ref={inputRef} type="search" aria-label="A’zolarni qidirish" placeholder="Username, ism-familiya yoki Telegram ID" value={search} onChange={(event) => setSearch(event.target.value)} /></label>
      <div className="member-result-list" role="list" aria-busy={members.isFetching}>
        {members.isLoading || (members.isFetching && !members.data) ? <SearchState kind="loading" message="A’zolar indeksidan qidirilmoqda" />
          : members.error ? <SearchState kind="error" message="A’zolarni yuklab bo‘lmadi" />
          : !members.data?.items.length ? <SearchState kind="empty" message="Mos a’zo topilmadi. A’zo guruhda xabar yuborgach indeksga tushadi." />
          : members.data.items.slice(0, 20).map((member) => <button type="button" key={member.user_id} aria-label={`${displayName(member)}, @${member.username ?? "username yo‘q"}, ID ${member.user_id}`} onClick={() => select(member)}><span className="avatar">{initials(displayName(member))}</span><span><strong>{displayName(member)}</strong><small>@{member.username ?? "username yo‘q"} · ID {member.user_id}</small></span><span className={`badge badge-${member.is_admin ? "lime" : "neutral"}`}>{member.status}</span></button>)}
      </div>
    </section>
  </div>;
}

function SearchState({ kind, message }: { kind: "loading" | "empty" | "error"; message: string }) {
  return <div className={`state-view state-${kind}`} role={kind === "error" ? "alert" : "status"}>{kind === "loading" ? <span className="spinner" /> : kind === "error" ? <X /> : <Search />}<strong>{kind === "loading" ? "Yuklanmoqda" : kind === "error" ? "Xato yuz berdi" : "A’zo topilmadi"}</strong><p>{message}</p></div>;
}
