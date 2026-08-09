import { Navigate, Route, Routes } from "react-router-dom";
import { useApp } from "./app-context";
import { AppShell, Button, StateView } from "./components";
import { telegramInitData } from "./telegram";
import { ActivityScreen } from "./screens/ActivityScreen";
import { ModerationScreen } from "./screens/ModerationScreen";
import { MoreScreen } from "./screens/MoreScreen";
import { OverviewScreen } from "./screens/OverviewScreen";
import { ProtectionScreen } from "./screens/ProtectionScreen";

export function App() {
  const { loading, error, chatId, demo } = useApp();
  if (!demo && !telegramInitData()) return <AuthRequired />;
  if (loading) return <div className="standalone-state"><StateView kind="loading" message="Telegram sessiyasi va guruhlar tekshirilmoqda" /></div>;
  if (error) return <div className="standalone-state"><StateView kind="error" message={error.message} action={<Button onClick={() => window.location.reload()}>Qayta urinish</Button>} /></div>;
  if (!chatId) return <div className="standalone-state"><StateView kind="empty" title="Boshqariladigan guruh topilmadi" message="Botni guruhga admin qilib qo‘shing va guruhda kamida bitta xabar yuboring." /></div>;

  return (
    <AppShell>
      <Routes>
        <Route path="/overview" element={<OverviewScreen />} />
        <Route path="/moderation" element={<ModerationScreen />} />
        <Route path="/protection" element={<ProtectionScreen />} />
        <Route path="/activity" element={<ActivityScreen />} />
        <Route path="/more" element={<MoreScreen />} />
        <Route path="*" element={<Navigate to="/overview" replace />} />
      </Routes>
    </AppShell>
  );
}

function AuthRequired() {
  return <div className="auth-gate"><div className="auth-mark"><ShieldIcon /></div><span className="eyebrow">CHEKLABOT MINI APP</span><h1>Telegram orqali oching</h1><p>Xavfsiz admin sessiyasi Telegram Mini App initData orqali tasdiqlanadi.</p><Button onClick={() => window.Telegram?.WebApp?.close()}>Telegramga qaytish</Button></div>;
}

function ShieldIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2 20 5v6c0 5.1-3.4 9.7-8 11-4.6-1.3-8-5.9-8-11V5l8-3Z" fill="none" stroke="currentColor" strokeWidth="2"/><path d="m8.5 12 2.2 2.2 4.8-5" fill="none" stroke="currentColor" strokeWidth="2"/></svg>;
}
