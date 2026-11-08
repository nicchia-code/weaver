import { Gauge } from "lucide-react";

import type { UsageLimitStatus } from "@/bindings/UsageLimitStatus";
import type { UsageStatus } from "@/bindings/UsageStatus";
import { formatReset } from "@/components/usage-dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useOmp } from "@/hooks/use-omp";
import { useUsageStatus } from "@/hooks/use-usage";
import { cn } from "@/lib/utils";

/** What the summary says: the limit closest to being used up, or why there is none to show. */
export type SummaryView =
  | { kind: "limit"; percent: number | null; name: string; reset: string | null; status: UsageLimitStatus }
  | { kind: "message"; text: string };

const STATUS_WORDS: Record<UsageLimitStatus, string> = { ok: "", almostUsedUp: "almost used up", usedUp: "used up" };

/**
 * The summary for the status the backend last sent (`requirements/ui#usage`). The limits read
 * earlier stay shown in every case, a reading included; without any, the text says what is going
 * on. `ompReady` is `null` while the status of omp is not known yet.
 */
export function summaryView(status: UsageStatus | null, ompReady: boolean | null, now: number): SummaryView {
  const summary = status?.summary ?? null;
  if (summary) {
    return {
      kind: "limit",
      percent: summary.usedFraction === null ? null : Math.round(summary.usedFraction * 100),
      name: summary.label,
      reset: formatReset(summary.resetsAt, now),
      status: summary.status,
    };
  }
  if (ompReady === false) return { kind: "message", text: "Usage needs the agent" };
  const usage = status?.usage ?? null;
  if (usage !== null) return { kind: "message", text: "No usage limits" };
  if (status?.phase === "reading") return { kind: "message", text: "Reading usage…" };
  if (status?.phase === "failed") return { kind: "message", text: "Could not read usage" };
  return { kind: "message", text: "Open to read usage" };
}

/** The summary as one text: `17% used, Claude 5 Hour · Resets in 2 h 10 min`; the state is in words too, never in colour alone. */
export function summaryText(view: SummaryView): string {
  if (view.kind === "message") return view.text;
  const where = [view.name, view.reset].filter((part) => part !== null).join(" · ");
  const used = view.percent === null ? null : `${view.percent}% used`;
  return [used, STATUS_WORDS[view.status] || null, where].filter((part) => part !== null).join(", ");
}

// Neutral (Ink 2 on `#EFEDE7`), Peach when almost used up, Rose when used up.
const BAR_TONE: Record<UsageLimitStatus, string> = { ok: "bg-ink-2", almostUsedUp: "bg-peach", usedUp: "bg-rose" };
const ICON_TONE: Record<UsageLimitStatus, string> = { ok: "", almostUsedUp: "text-peach-ink", usedUp: "text-rose-ink" };

export interface UsageSummaryProps {
  /** Below 1024 px the sidebar shows icons only: the summary is a `Gauge` button. */
  compact: boolean;
  /** `developer.OPEN_USAGE`; `origin` is the button, where the focus goes back when Usage closes. */
  onOpen: (origin: HTMLElement) => void;
}

/**
 * The summary of the usage limits at the bottom of the sidebar (`requirements/ui#usage`): the whole
 * box is one button that opens the Usage dialog. It shows the limits read last, app-wide, and
 * changes as soon as a reading ends; nothing here reads.
 */
export function UsageSummary({ compact, onOpen }: UsageSummaryProps) {
  const status = useUsageStatus();
  const { status: omp } = useOmp();
  const view = summaryView(status, omp === null ? null : omp.state === "ready", Date.now());
  const text = summaryText(view);
  const name = `Usage: ${text}`;

  if (compact) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label={name}
            data-usage-summary
            onClick={(event) => onOpen(event.currentTarget)}
            className={cn(
              "relative flex h-[34px] w-full items-center justify-center rounded-md text-ink-2 transition-colors duration-150 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              view.kind === "limit" && ICON_TONE[view.status],
            )}
          >
            <Gauge className="size-4 shrink-0" aria-hidden />
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">{name}</TooltipContent>
      </Tooltip>
    );
  }

  const fraction = view.kind === "limit" && view.percent !== null ? Math.min(Math.max(view.percent, 0), 100) : null;
  return (
    <button
      type="button"
      aria-label={name}
      data-usage-summary
      onClick={(event) => onOpen(event.currentTarget)}
      className="flex w-full flex-col gap-2.5 rounded-xl bg-surface p-3 text-left shadow-ring transition-colors duration-150 hover:bg-lane focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span className="flex items-baseline justify-between gap-2">
        <span className="text-xs font-medium">Usage</span>
        {fraction !== null ? <span className="text-xs tabular-nums text-muted-foreground">{fraction}%</span> : null}
      </span>
      {view.kind === "limit" ? (
        <>
          {fraction !== null ? (
            <span aria-hidden className="block h-1.5 overflow-hidden rounded-full bg-line-soft">
              <span data-usage-bar={view.status} className={cn("block h-full rounded-full", BAR_TONE[view.status])} style={{ width: `${fraction}%` }} />
            </span>
          ) : null}
          <span className="truncate text-xs text-muted-foreground">{[view.name, view.reset].filter((part) => part !== null).join(" · ")}</span>
        </>
      ) : (
        <span className="truncate text-xs text-muted-foreground">{view.text}</span>
      )}
    </button>
  );
}
