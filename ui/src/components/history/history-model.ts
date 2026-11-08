// What the History view works out from the entries the backend sends (newest first): the period
// filter and the grouping by local day. Times are local; the backend holds UTC instants.

import type { HistoryEntry } from "@/bindings/HistoryEntry";
import { formatCalendarDate, formatDate, formatTime, formatWeekday } from "@/lib/format";

export type Period = "today" | "last-7-days" | "last-30-days" | "all";

export interface PeriodChoice {
  id: Period;
  label: string;
  /** Days before today where the period starts (`null`: no start). */
  daysBack: number | null;
}

/** The choices of the period filter, in order. A period has both bounds: its first local midnight and now. */
export const PERIODS: readonly PeriodChoice[] = [
  { id: "today", label: "Today", daysBack: 0 },
  { id: "last-7-days", label: "Last 7 days", daysBack: 6 },
  { id: "last-30-days", label: "Last 30 days", daysBack: 29 },
  { id: "all", label: "All time", daysBack: null },
];

/** The period shown at each opening of the window. */
export const DEFAULT_PERIOD: Period = "last-7-days";

/** Local midnight of the day `daysBack` days before the day of `time`. */
export function localMidnight(time: number, daysBack = 0): number {
  const date = new Date(time);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() - daysBack).getTime();
}

/** Whether an entry dated `time` is in `period` at `now`: from the first local midnight to now; a future date only in `all`. */
export function inPeriod(time: number, period: Period, now: number): boolean {
  const choice = PERIODS.find((candidate) => candidate.id === period);
  if (!choice || choice.daysBack === null) return true;
  return time >= localMidnight(now, choice.daysBack) && time <= now;
}

/** The date of an entry: when it was written or last updated. */
export function entryTime(entry: Pick<HistoryEntry, "updated">): number {
  return Date.parse(entry.updated);
}

export interface DayGroup {
  /** Local midnight of the day, as the key of the group. */
  day: number;
  /** `Today`, `Yesterday` or the weekday (`Friday`). */
  name: string;
  /** The date written out after the name: `Monday, 2 November 2026` for Today and Yesterday, `30 October 2026` after a weekday. */
  date: string;
  /** `· 2 entries`, `· 1 entry`. */
  count: string;
  entries: HistoryEntry[];
}

function dayNames(day: number, now: number): { name: string; date: string } {
  if (day === localMidnight(now)) return { name: "Today", date: formatDate(day) };
  if (day === localMidnight(now, 1)) return { name: "Yesterday", date: formatDate(day) };
  return { name: formatWeekday(day), date: formatCalendarDate(day) };
}

/** The entries of `period`, grouped by local day in the order they come (newest first). */
export function groupByDay(entries: readonly HistoryEntry[], period: Period, now: number): DayGroup[] {
  const groups = new Map<number, HistoryEntry[]>();
  for (const entry of entries) {
    const time = entryTime(entry);
    if (!inPeriod(time, period, now)) continue;
    const day = localMidnight(time);
    const group = groups.get(day);
    if (group) group.push(entry);
    else groups.set(day, [entry]);
  }
  return [...groups].map(([day, inDay]) => ({
    day,
    ...dayNames(day, now),
    count: `· ${inDay.length} ${inDay.length === 1 ? "entry" : "entries"}`,
    entries: inDay,
  }));
}

export interface PeriodTotals {
  /** The number of entries of the period. */
  delivered: number;
  /** The number of Tasks the entries cite. */
  tasksDone: number;
  /** The date of the newest entry (`Today, 16:07`, `Yesterday, 16:07`, else the date written out); `null` when there is none. */
  lastDelivery: string | null;
}

/** The totals of the entries of `period`, worked out from the groups `groupByDay` made. */
export function periodTotals(groups: readonly DayGroup[], now: number): PeriodTotals {
  let delivered = 0;
  let tasksDone = 0;
  let newest: number | null = null;
  for (const group of groups) {
    for (const entry of group.entries) {
      delivered += 1;
      tasksDone += entry.tasks.length;
      const time = entryTime(entry);
      if (newest === null || time > newest) newest = time;
    }
  }
  if (newest === null) return { delivered, tasksDone, lastDelivery: null };
  const day = localMidnight(newest);
  const lastDelivery =
    day === localMidnight(now)
      ? `Today, ${formatTime(newest)}`
      : day === localMidnight(now, 1)
        ? `Yesterday, ${formatTime(newest)}`
        : formatDate(newest);
  return { delivered, tasksDone, lastDelivery };
}
