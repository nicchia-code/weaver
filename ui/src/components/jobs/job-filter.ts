import type { ListRow } from "@/components/ask/ask-model";

/** The segments of the filter at the top of the Jobs list. */
export type JobFilter = "all" | "active" | "ended";

export const JOB_FILTERS: readonly { id: JobFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "active", label: "Active" },
  { id: "ended", label: "Ended" },
];

/**
 * A job queued, running, waiting or suspended, or an Ask more conversation whose agent works or
 * waits. An idle conversation is neither active nor ended: it shows only under `All`.
 */
export function isActive(row: ListRow): boolean {
  if (row.type === "ask") return row.ask.status === "running" || row.ask.status === "waiting";
  return row.job.state === "queued" || row.job.state === "running" || row.job.state === "waiting" || row.job.state === "suspended";
}

/** A job done, failed or stopped. */
export function isEnded(row: ListRow): boolean {
  return row.type === "job" && (row.job.state === "done" || row.job.state === "failed" || row.job.state === "stopped");
}

export function filterRows(rows: readonly ListRow[], filter: JobFilter): ListRow[] {
  if (filter === "all") return [...rows];
  return rows.filter(filter === "active" ? isActive : isEnded);
}

export function countRows(rows: readonly ListRow[]): Record<JobFilter, number> {
  return { all: rows.length, active: rows.filter(isActive).length, ended: rows.filter(isEnded).length };
}
