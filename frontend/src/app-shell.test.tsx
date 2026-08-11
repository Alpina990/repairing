// @vitest-environment jsdom

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { AppProvider } from "./app-context";

function renderApp(path = "/overview") {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <AppProvider>
          <App />
        </AppProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("AppShell interactions", () => {
  beforeEach(() => localStorage.clear());
  afterEach(cleanup);

  it("opens global member search and navigates to the selected member", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: "A’zolarni qidirish" }));
    const search = screen.getByRole("searchbox", { name: "A’zolarni qidirish" });
    await user.type(search, "Alisher Karimov");
    await user.click(await screen.findByRole("button", { name: /Alisher Karimov/ }));

    expect(await screen.findByRole("heading", { name: "Moderatsiya" })).not.toBeNull();
    expect(await screen.findByText("Alisher Karimov")).not.toBeNull();
  });

  it("switches the active group from the header", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: "Guruhni almashtirish" }));
    await user.click(screen.getByRole("option", { name: /Frontend UZ/ }));

    expect(screen.getByRole("button", { name: "Guruhni almashtirish" }).textContent).toContain("Frontend UZ");
    expect(localStorage.getItem("cheklabot.chat_id")).toBe("-10011223344");
  });

  it("finds a member by Telegram-style @username on the moderation screen", async () => {
    const user = userEvent.setup();
    renderApp("/moderation");

    const search = await screen.findByRole("searchbox", { name: "A’zolarni qidirish" });
    await user.type(search, "@alisher");
    await user.click(await screen.findByRole("option", { name: /Alisher Karimov/ }));

    expect(await screen.findByText("Alisher Karimov")).not.toBeNull();
    expect(screen.getByText(/ID 884201/)).not.toBeNull();
  });

  it("clears the selected member when the active group changes", async () => {
    const user = userEvent.setup();
    renderApp("/moderation?member=884201");
    expect(await screen.findByText("Alisher Karimov")).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Guruhni almashtirish" }));
    await user.click(screen.getByRole("option", { name: /Frontend UZ/ }));

    expect(await screen.findByText("Username, yozilgan ism yoki Telegram ID orqali foydalanuvchini toping")).not.toBeNull();
    expect(screen.queryByText("Alisher Karimov")).toBeNull();
  });

  it("executes moderation and protection mutations from their buttons", async () => {
    const user = userEvent.setup();
    renderApp("/moderation");

    await user.type(await screen.findByRole("searchbox", { name: "A’zolarni qidirish" }), "alisher");
    await user.click(await screen.findByRole("option", { name: /Alisher Karimov/ }));
    await user.click(screen.getByRole("button", { name: "Ogohlantirish yuborish" }));
    expect(await screen.findByText(/Amal muvaffaqiyatli bajarildi/)).not.toBeNull();

    await user.click(screen.getByRole("link", { name: "Himoya" }));
    await user.click(await screen.findByRole("button", { name: "Sozlamalarni saqlash" }));
    expect(await screen.findByText("Himoya siyosati saqlandi")).not.toBeNull();
  });

  it("adds and removes a blocklist term through the content controls", async () => {
    const user = userEvent.setup();
    renderApp("/protection");

    await user.click(await screen.findByRole("radio", { name: "Kontent" }));
    await user.type(await screen.findByRole("textbox", { name: "Yangi taqiqlangan ibora" }), "phishing test");
    await user.click(screen.getByRole("button", { name: "Qo‘shish" }));
    expect(await screen.findByText("phishing test")).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "phishing test iborasini o‘chirish" }));
    await user.click(screen.getByRole("button", { name: "O‘chirish" }));
    expect(await screen.findByText("Ibora olib tashlandi")).not.toBeNull();
    await waitFor(() => expect(screen.queryByText("phishing test")).toBeNull());
  });

  it("opens activity filters, incident actions, and the command guide", async () => {
    const user = userEvent.setup();
    renderApp("/activity");

    await user.click(await screen.findByRole("button", { name: /Filtr/ }));
    expect(screen.getByText("Boshlanish")).not.toBeNull();
    await user.click(screen.getByRole("radio", { name: "Incidentlar" }));
    await user.click(await screen.findByRole("button", { name: /anti flood.*#1042/ }));
    await user.click(screen.getByRole("button", { name: "Qabul qilish" }));
    expect(await screen.findByText("Incident holati yangilandi")).not.toBeNull();

    await user.click(screen.getByRole("link", { name: "Boshqa" }));
    await user.click(await screen.findByRole("button", { name: /Telegram buyruqlari/ }));
    expect(screen.getByText("/start")).not.toBeNull();
    expect(screen.getByText("/setflood <limit> [s] [action]")).not.toBeNull();
  });
});
