import {
  Activity,
  AlertTriangle,
  Check,
  Grid2X2,
  MoreHorizontal,
  Search,
  ShieldCheck,
  UserRoundCog,
  X,
} from "lucide-react";
import { useEffect, useRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { useApp } from "./app-context";
import { AppHeader } from "./AppHeader";
import { haptic } from "./telegram";

export function AppShell({ children }: { children: ReactNode }) {
  const { demo } = useApp();
  return (
    <div className="viewport">
      <div className="phone-shell">
        {demo && <div className="demo-ribbon">LOCAL DEMO · Telegramga amal yuborilmaydi</div>}
        <AppHeader />
        <main className="app-main">{children}</main>
        <nav className="bottom-nav" aria-label="Asosiy navigatsiya">
          <NavItem to="/overview" icon={<Grid2X2 />} label="Asosiy" />
          <NavItem to="/moderation" icon={<UserRoundCog />} label="Nazorat" />
          <NavItem to="/protection" icon={<ShieldCheck />} label="Himoya" />
          <NavItem to="/activity" icon={<Activity />} label="Faollik" />
          <NavItem to="/more" icon={<MoreHorizontal />} label="Boshqa" />
        </nav>
      </div>
    </div>
  );
}

function NavItem({ to, icon, label }: { to: string; icon: ReactNode; label: string }) {
  return (
    <NavLink className={({ isActive }) => `nav-item${isActive ? " active" : ""}`} to={to} onClick={() => haptic("tap")}>
      {icon}
      <span>{label}</span>
    </NavLink>
  );
}

export function PageHeader({ title, subtitle, action }: { title: string; subtitle?: string; action?: ReactNode }) {
  return <div className="page-heading"><div><h1>{title}</h1>{subtitle && <p>{subtitle}</p>}</div>{action}</div>;
}

export function SectionHeader({ title, meta, action }: { title: string; meta?: string; action?: ReactNode }) {
  return <div className="section-heading"><div><h2>{title}</h2>{meta && <p>{meta}</p>}</div>{action}</div>;
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "lime" | "danger" | "success" }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Button({ className = "", variant = "primary", onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "danger" | "ghost" }) {
  return <button {...props} className={`button button-${variant} ${className}`} onClick={(event) => { haptic("tap"); onClick?.(event); }} />;
}

export function IconButton({ label, children, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; children: ReactNode }) {
  return <button {...props} type={props.type ?? "button"} className="icon-button" aria-label={label}>{children}</button>;
}

export function Toggle({ checked, onChange, disabled, label }: { checked: boolean; onChange?(value: boolean): void; disabled?: boolean; label: string }) {
  return (
    <button className={`toggle${checked ? " checked" : ""}`} type="button" role="switch" aria-checked={checked} aria-label={label} disabled={disabled} onClick={() => onChange?.(!checked)}>
      <span />
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange(value: T): void; label: string }) {
  return <div className="segmented" role="radiogroup" aria-label={label}>{options.map((option) => <button key={option.value} type="button" role="radio" aria-checked={value === option.value} className={value === option.value ? "active" : ""} onClick={() => onChange(option.value)}>{option.label}</button>)}</div>;
}

export function SearchField(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <label className="search-field"><Search size={18} aria-hidden="true" /><span className="sr-only">{props["aria-label"] ?? props.placeholder}</span><input {...props} /></label>;
}

export function StateView({ kind, title, message, action }: { kind: "loading" | "empty" | "error"; title?: string; message?: string; action?: ReactNode }) {
  return <div className={`state-view state-${kind}`} role={kind === "error" ? "alert" : "status"}>{kind === "error" ? <AlertTriangle /> : kind === "empty" ? <Search /> : <span className="spinner" />}<strong>{title ?? (kind === "loading" ? "Yuklanmoqda" : kind === "empty" ? "Ma’lumot topilmadi" : "Xato yuz berdi")}</strong>{message && <p>{message}</p>}{action}</div>;
}

export function StatusDot({ ok }: { ok: boolean }) {
  return <span className={`status-dot ${ok ? "ok" : "bad"}`} aria-label={ok ? "Ishlayapti" : "Muammo bor"}>{ok ? <Check size={12} /> : <X size={12} />}</span>;
}

export function ConfirmDialog({ open, title, message, confirmLabel, danger, pending, onConfirm, onClose }: { open: boolean; title: string; message: string; confirmLabel: string; danger?: boolean; pending?: boolean; onConfirm(): void; onClose(): void }) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => { if (open) closeRef.current?.focus(); }, [open]);
  if (!open) return null;
  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}><div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title"><button ref={closeRef} className="dialog-close" aria-label="Yopish" onClick={onClose}><X /></button><div className={`dialog-icon${danger ? " danger" : ""}`}><AlertTriangle /></div><h2 id="confirm-title">{title}</h2><p>{message}</p><div className="dialog-actions"><Button variant="secondary" onClick={onClose} disabled={pending}>Bekor qilish</Button><Button variant={danger ? "danger" : "primary"} onClick={onConfirm} disabled={pending}>{pending ? "Bajarilmoqda…" : confirmLabel}</Button></div></div></div>;
}

export function Toast({ message, tone = "success", onClose }: { message?: string; tone?: "success" | "error"; onClose(): void }) {
  useEffect(() => { if (!message) return; const id = window.setTimeout(onClose, 3500); return () => window.clearTimeout(id); }, [message, onClose]);
  if (!message) return null;
  return <div className={`toast toast-${tone}`} role="status">{tone === "success" ? <Check /> : <AlertTriangle />}<span>{message}</span><button onClick={onClose} aria-label="Yopish"><X /></button></div>;
}
