import { useQuery } from "@tanstack/react-query";
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { createHttpApi, type CheklaApi } from "./api";
import { createDemoApi } from "./demo";
import { telegramInitData } from "./telegram";
import type { ManagedChat, MeResponse } from "./types";

interface AppContextValue {
  api: CheklaApi;
  demo: boolean;
  me?: MeResponse;
  chats: ManagedChat[];
  chat?: ManagedChat;
  chatId?: number;
  setChatId(id: number): void;
  loading: boolean;
  error?: Error;
}

const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const allowDemo = import.meta.env.DEV || import.meta.env.VITE_ALLOW_DEMO === "true";
  const demo = allowDemo && !telegramInitData();
  const api = useMemo(() => (demo ? createDemoApi() : createHttpApi()), [demo]);
  const [chatId, setChatIdState] = useState<number | undefined>(() => {
    const stored = localStorage.getItem("cheklabot.chat_id");
    return stored ? Number(stored) : undefined;
  });
  const meQuery = useQuery({ queryKey: ["me", demo], queryFn: api.getMe, retry: false });
  const chatsQuery = useQuery({ queryKey: ["chats", demo], queryFn: api.getChats, retry: false });
  const chats = chatsQuery.data?.items ?? [];

  useEffect(() => {
    if ((!chatId || !chats.some((chat) => chat.chat_id === chatId)) && chats[0]) {
      setChatIdState(chats[0].chat_id);
    }
  }, [chatId, chats]);

  const setChatId = (id: number) => {
    localStorage.setItem("cheklabot.chat_id", String(id));
    setChatIdState(id);
  };

  const error = (meQuery.error || chatsQuery.error) as Error | null;
  return (
    <AppContext.Provider
      value={{
        api,
        demo,
        me: meQuery.data,
        chats,
        chat: chats.find((item) => item.chat_id === chatId),
        chatId,
        setChatId,
        loading: meQuery.isLoading || chatsQuery.isLoading,
        error: error ?? undefined,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp(): AppContextValue {
  const value = useContext(AppContext);
  if (!value) throw new Error("useApp must be used inside AppProvider");
  return value;
}
