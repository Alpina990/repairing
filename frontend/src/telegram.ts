declare global {
  interface Window {
    Telegram?: {
      WebApp?: {
        initData: string;
        colorScheme?: "light" | "dark";
        ready(): void;
        expand(): void;
        close(): void;
        HapticFeedback?: {
          impactOccurred(style: "light" | "medium" | "heavy"): void;
          notificationOccurred(type: "error" | "success" | "warning"): void;
        };
      };
    };
  }
}

export function telegramInitData(): string {
  return window.Telegram?.WebApp?.initData ?? "";
}

export function initializeTelegram(): void {
  const app = window.Telegram?.WebApp;
  app?.ready();
  app?.expand();
}

export function haptic(type: "tap" | "success" | "error"): void {
  const feedback = window.Telegram?.WebApp?.HapticFeedback;
  if (type === "tap") feedback?.impactOccurred("light");
  else feedback?.notificationOccurred(type);
}
