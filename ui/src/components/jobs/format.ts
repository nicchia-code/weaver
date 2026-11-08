/** `HH:MM` today, else `D MMM HH:MM`, in local time. */
export function startedText(iso: string, now: Date = new Date()): string {
  const at = new Date(iso);
  const time = `${String(at.getHours()).padStart(2, "0")}:${String(at.getMinutes()).padStart(2, "0")}`;
  if (at.toDateString() === now.toDateString()) return time;
  const month = at.toLocaleString("en-GB", { month: "short" });
  return `${at.getDate()} ${month} ${time}`;
}

/** `Started today at HH:MM`, else `Started D MMM at HH:MM`, in local time. */
export function startedLine(iso: string, now: Date = new Date()): string {
  const text = startedText(iso, now);
  return `Started ${text.length > 5 ? text.slice(0, -6) : "today"} at ${text.slice(-5)}`;
}

/** `45s`, `4m 05s`, `1h 12m`. */
export function runningText(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}
