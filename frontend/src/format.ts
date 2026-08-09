export function formatDuration(seconds?: number | null): string {
  if (seconds == null) return "—";
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86_400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86_400)}d`;
}

export function relativeTime(value?: string | null): string {
  if (!value) return "—";
  const delta = Math.max(0, Date.now() - new Date(value).getTime()) / 1000;
  if (delta < 60) return "hozir";
  if (delta < 3600) return `${Math.floor(delta / 60)}m`;
  if (delta < 86_400) return `${Math.floor(delta / 3600)}h`;
  return `${Math.floor(delta / 86_400)}d`;
}

export function displayName(member: Pick<import("./types").ChatMember, "first_name" | "last_name">): string {
  return [member.first_name, member.last_name].filter(Boolean).join(" ");
}

export function initials(value: string): string {
  return value
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("");
}
