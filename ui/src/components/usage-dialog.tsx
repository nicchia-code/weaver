import { Loader, RotateCw } from "lucide-react";

import type { ProviderUsage } from "@/bindings/ProviderUsage";
import type { Usage } from "@/bindings/Usage";
import type { UsageLimit } from "@/bindings/UsageLimit";
import type { UsageNoLimits } from "@/bindings/UsageNoLimits";
import { boxFace } from "@/components/omp-status";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { useOmp } from "@/hooks/use-omp";
import { useUsage } from "@/hooks/use-usage";
import { formatDate, formatDuration, formatTime } from "@/lib/format";
import { TONE_DOT, TONE_TEXT } from "@/lib/tones";
import { cn } from "@/lib/utils";

const DAY = 24 * 60 * 60 * 1000;

const NO_LIMITS_TEXT: Record<UsageNoLimits, string> = {
  noUsageData: "No usage data",
  loginExpired: "Login expired",
  notReported: "Not reported by the agent",
};

// Peach for almost used up, Rose for used up; the others stay neutral.
const STATUS_TEXT = { ok: "", almostUsedUp: TONE_TEXT.peach, usedUp: TONE_TEXT.rose } as const;
const STATUS_BAR = { ok: "", almostUsedUp: TONE_DOT.peach, usedUp: TONE_DOT.rose } as const;
const STATUS_LABEL = { almostUsedUp: "Almost used up", usedUp: "Used up" } as const;

const plain = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
const money = new Intl.NumberFormat("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

/** The value of a limit by unit (`requirements/ui#usage`): `17% used`, `3,437 / 100,000 requests`, `$0.50 / $1,000.00`. */
export function formatLimitValue(limit: UsageLimit): string {
  const fraction = limit.usedFraction;
  if (fraction === null) return "Usage unknown";
  const unit = limit.unit;
  const used = limit.used ?? (limit.limit !== null ? fraction * limit.limit : null);
  if (unit === "percent") return `${plain.format(limit.used ?? fraction * 100)}% used`;
  // Without the amounts (omp gave only a fraction) the fraction is all there is.
  if (used === null) return `${plain.format(fraction * 100)}% used`;
  const amount = (value: number) => (unit === "usd" ? `$${money.format(value)}` : plain.format(value));
  const suffix = unit === "usd" || unit === null ? "" : ` ${unit === "minutes" ? "min" : unit}`;
  return limit.limit === null ? `${amount(used)}${suffix}` : `${amount(used)} / ${amount(limit.limit)}${suffix}`;
}

/** `Resets in <duration>` under 24 h, else `Resets on <date>`; `null` when omp gives no time. */
export function formatReset(resetsAt: number | null, now: number): string | null {
  if (resetsAt === null) return null;
  return resetsAt - now < DAY ? `Resets in ${formatDuration(resetsAt - now)}` : `Resets on ${formatDate(resetsAt)}`;
}

function LimitRow({ limit, now }: { limit: UsageLimit; now: number }) {
  const value = formatLimitValue(limit);
  const reset = formatReset(limit.resetsAt, now);
  const status = limit.status;
  return (
    <li className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between gap-3 text-sm">
        <span className="min-w-0 truncate">{limit.label}</span>
        <span className={cn("shrink-0 tabular-nums", status === "ok" ? "text-muted-foreground" : STATUS_TEXT[status])}>{value}</span>
      </div>
      {limit.usedFraction !== null ? (
        <Progress value={limit.usedFraction * 100} aria-label={`${limit.label}: ${value}`} indicatorClassName={STATUS_BAR[status]} />
      ) : null}
      {reset || status !== "ok" ? (
        <div className="flex items-baseline justify-between gap-3 text-xs">
          <span className="text-muted-foreground">{reset}</span>
          {status !== "ok" ? <span className={cn("font-medium", STATUS_TEXT[status])}>{STATUS_LABEL[status]}</span> : null}
        </div>
      ) : null}
    </li>
  );
}

function ProviderBlock({ provider, now }: { provider: ProviderUsage; now: number }) {
  const headingId = `usage-provider-${provider.id}`;
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2">
      <h3 id={headingId} className="text-sm font-bold">
        {provider.name}
      </h3>
      {provider.limits.length > 0 ? (
        <ul className="flex flex-col gap-3">
          {provider.limits.map((limit, index) => (
            <LimitRow key={`${limit.label}-${index}`} limit={limit} now={now} />
          ))}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground">{provider.noLimits ? NO_LIMITS_TEXT[provider.noLimits] : NO_LIMITS_TEXT.notReported}</p>
      )}
    </section>
  );
}

function Limits({ usage }: { usage: Usage }) {
  const now = Date.now();
  const none = usage.providers.every((provider) => provider.limits.length === 0);
  return (
    <div className="flex flex-col gap-4">
      {none ? <p className="text-sm text-muted-foreground">No usage limits to show.</p> : null}
      {usage.providers.map((provider) => (
        <ProviderBlock key={provider.id} provider={provider} now={now} />
      ))}
    </div>
  );
}

function Updating() {
  return (
    <span className="inline-flex items-center gap-1">
      <Loader aria-hidden className="size-3 animate-spin motion-reduce:animate-none" />
      Updating…
    </span>
  );
}

/** The muted line under the title: when omp read the data, and `Updating…` while a reading runs. */
function TimeLine({ usage, updating }: { usage: Usage | null; updating: boolean }) {
  const at = usage?.fetchedAt ?? null;
  if (at === null && !updating) return null;
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground">
      {at !== null ? <span>{`Updated at ${formatTime(at)}`}</span> : null}
      {updating ? <Updating /> : null}
    </p>
  );
}

function UsageBody({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { status: omp } = useOmp();
  const ompReady = omp?.state === "ready";
  const { status, current, read } = useUsage(ompReady);

  if (!ompReady) {
    const face = boxFace(omp, "elsewhere");
    return (
      <>
        <DialogHeader>
          <DialogTitle>Usage</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <div role="status" aria-live="polite" className="flex flex-col items-start gap-3 text-sm">
            <p>{face.text}</p>
            {face.hint ? <p className="text-muted-foreground">{face.hint}</p> : null}
            <Button size="sm" variant="outline" onClick={onOpenSettings}>
              Open Settings
            </Button>
          </div>
        </DialogBody>
      </>
    );
  }

  // The limits read last stay shown while new ones are read and when reading fails.
  const earlier = (current ?? status)?.usage ?? null;
  const reading = current === null || current.phase === "reading" || (current.phase === "idle" && current.usage === null);
  const failed = !reading && current?.phase === "failed";
  return (
    <>
      <DialogHeader>
        <DialogTitle>Usage</DialogTitle>
        {earlier || reading ? <TimeLine usage={earlier} updating={reading && earlier !== null} /> : null}
      </DialogHeader>
      <DialogBody aria-busy={reading ? "true" : "false"}>
        {reading && earlier === null ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
            <Loader aria-hidden className="size-4 animate-spin motion-reduce:animate-none" />
            Reading usage…
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {failed ? (
              <Alert variant="error">
                <div className="min-w-0 flex-1">{`Could not read usage: ${(current?.failure ?? "the agent did not say why").replace(/\.$/, "")}.`}</div>
                <Button size="sm" variant="outline" onClick={read}>
                  <RotateCw aria-hidden />
                  Retry
                </Button>
              </Alert>
            ) : null}
            {earlier ? <Limits usage={earlier} /> : null}
          </div>
        )}
      </DialogBody>
    </>
  );
}

export interface UsageDialogProps {
  open: boolean;
  /** `developer.CLOSE_USAGE`: Esc, `X` or a click outside. */
  onClose: () => void;
  /** `Open Settings` in Waiting for omp: the dialog closes and the window shows Settings. */
  onOpenSettings: () => void;
  /** Where the focus goes when the dialog closes (where it was when the dialog was opened). */
  onCloseAutoFocus?: (event: Event) => void;
}

/**
 * The Machine `usage` in the window of a project: how much is left of the usage limits of the
 * providers omp is connected to, and when each limit resets (`requirements/ui#usage`). Its content
 * only exists while it is open, so each opening starts a reading; the readings of the timer reach
 * it, and the summary in the sidebar, as statuses like any other.
 */
export function UsageDialog({ open, onClose, onOpenSettings, onCloseAutoFocus }: UsageDialogProps) {
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? undefined : onClose())}>
      <DialogContent className="max-w-[480px]" aria-describedby={undefined} onCloseAutoFocus={onCloseAutoFocus}>
        <UsageBody onOpenSettings={onOpenSettings} />
      </DialogContent>
    </Dialog>
  );
}
