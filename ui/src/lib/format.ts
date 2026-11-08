// Times and durations as the interface shows them. Times are local, `HH:MM` on 24 hours; dates
// are written out in English; none of it depends on the system's locale.

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"] as const;
const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
] as const;

const two = (n: number) => String(n).padStart(2, "0");

/** `07:05`: local, 24 hours. */
export function formatTime(time: Date | number): string {
  const date = new Date(time);
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** `2 November 2026`. */
export function formatCalendarDate(time: Date | number): string {
  const date = new Date(time);
  return `${date.getDate()} ${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}

/** `Monday`: the weekday in English. */
export function formatWeekday(time: Date | number): string {
  return WEEKDAYS[new Date(time).getDay()] as string;
}

/** `Monday, 2 November 2026`. */
export function formatDate(time: Date | number): string {
  return `${formatWeekday(time)}, ${formatCalendarDate(time)}`;
}

/** `less than a minute`, `<n> min`, `<h> h <m> min`: a length of time in words. */
export function formatDuration(milliseconds: number): string {
  const minutes = Math.floor(Math.max(0, milliseconds) / 60_000);
  if (minutes < 1) return "less than a minute";
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

/** When a job started: `HH:MM` today, else `D MMM HH:MM` (`2 Nov 07:05`). */
export function formatStarted(time: Date | number, now: Date | number): string {
  const date = new Date(time);
  const today = new Date(now);
  const sameDay =
    date.getFullYear() === today.getFullYear() &&
    date.getMonth() === today.getMonth() &&
    date.getDate() === today.getDate();
  return sameDay ? formatTime(date) : `${date.getDate()} ${MONTHS[date.getMonth()]?.slice(0, 3)} ${formatTime(date)}`;
}

/** A job's running time, counted in whole seconds: `45s`, `4m 05s`, `1h 12m`. */
export function formatRunning(milliseconds: number): string {
  const seconds = Math.floor(Math.max(0, milliseconds) / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${two(seconds % 60)}s`;
  return `${Math.floor(minutes / 60)}h ${two(minutes % 60)}m`;
}
