import { ChevronDown, ChevronRight } from "lucide-react";
import { useId, useRef, useState } from "react";

import type { HistoryEntry } from "@/bindings/HistoryEntry";
import type { HistoryTask } from "@/bindings/HistoryTask";
import { AskMoreButton } from "@/components/ask/ask-button";
import { CardId } from "@/components/card-id";
import { FormDialog } from "@/components/dialogs";
import {
  DEFAULT_PERIOD,
  entryTime,
  groupByDay,
  inPeriod,
  PERIODS,
  periodTotals,
  type DayGroup,
  type Period,
} from "@/components/history/history-model";
import { useHistory } from "@/components/history/use-history";
import { Markdown } from "@/components/markdown";
import { useProject, useReveal } from "@/components/project-context";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/view-header";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { formatDate, formatStarted, formatTime } from "@/lib/format";
import { plainText } from "@/lib/plain-text";

const SUBTITLE = "Everything that shipped, grouped as it was delivered.";

/** One box of the totals row: a small label over a large value. */
function TotalBox({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1 rounded-xl bg-lane px-4 py-3.5 ring-1 ring-inset ring-line">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-2xl font-semibold tracking-tight tabular-nums">{value}</dd>
    </div>
  );
}

function EntryRow({ entry, onOpen }: { entry: HistoryEntry; onOpen: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onOpen}
          className="flex w-full items-start gap-3.5 px-4 py-3.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
        >
          <span aria-hidden className="w-1 shrink-0 self-stretch rounded-full bg-border" />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-semibold">{entry.title}</span>
            <span className="line-clamp-2 text-[13px] leading-normal text-ink-2">{plainText(entry.summary)}</span>
          </span>
          <span className="flex shrink-0 flex-col items-end gap-1.5">
            <time dateTime={entry.updated} className="font-mono text-xs text-muted-foreground">
              {formatTime(entryTime(entry))}
            </time>
            <Badge variant="success">
              {entry.tasks.length} {entry.tasks.length === 1 ? "task" : "tasks"}
            </Badge>
          </span>
        </button>
      </TooltipTrigger>
      <TooltipContent>{entry.summary}</TooltipContent>
    </Tooltip>
  );
}

function DayList({ group, onOpen }: { group: DayGroup; onOpen: (entry: HistoryEntry) => void }) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2.5">
      <h2 id={headingId} className="text-[13px] font-semibold">
        {group.name} <span className="font-normal text-muted-foreground">{group.date} {group.count}</span>
      </h2>
      <ul className="flex flex-col overflow-hidden rounded-xl ring-1 ring-line">
        {group.entries.map((entry) => (
          <li key={entry.id} className="border-b border-accent last:border-b-0">
            <EntryRow entry={entry} onOpen={() => onOpen(entry)} />
          </li>
        ))}
      </ul>
    </section>
  );
}

/** One Task of an entry, from its copy: a row that expands to its body and refs, read-only. */
function TaskRow({ task, now }: { task: HistoryTask; now: number }) {
  const [expanded, setExpanded] = useState(false);
  const panelId = useId();
  const finished = task.finished === null ? null : formatStarted(Date.parse(task.finished), now);
  const Chevron = expanded ? ChevronDown : ChevronRight;
  return (
    <li className="rounded-lg shadow-ring">
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={panelId}
        onClick={() => setExpanded((value) => !value)}
        className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Chevron className="size-4 shrink-0" aria-hidden />
        <CardId permanentId={task.id} />
        <span className="min-w-0 flex-1 truncate">{task.title}</span>
        {task.label ? <Badge variant="outline">{task.label}</Badge> : null}
        {finished ? <span className="shrink-0 text-xs text-muted-foreground">Finished {finished}</span> : null}
      </button>
      <div id={panelId} hidden={!expanded} className="flex flex-col gap-2 border-t px-3 py-2">
        {task.body ? <Markdown holderHeadingLevel={3}>{task.body}</Markdown> : <p className="text-sm text-muted-foreground">No text.</p>}
        {task.refs.length > 0 ? (
          <ul aria-label="References" className="flex flex-wrap gap-1">
            {task.refs.map((ref) => (
              <li key={ref}>
                <Badge variant="muted" className="font-mono font-normal">
                  {ref}
                </Badge>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </li>
  );
}

/** The entry in a Dialog: title, date, summary, text, then its Tasks from the copies it keeps; `Ask more` on the whole entry in its footer. */
function EntryDialog({
  entry,
  now,
  onClose,
}: {
  entry: HistoryEntry | null;
  now: number;
  onClose: (open: boolean) => void;
}) {
  // The last entry shown keeps the dialog's content while it closes.
  const last = useRef<HistoryEntry | null>(null);
  if (entry) last.current = entry;
  const shown = entry ?? last.current;
  if (!shown) return null;
  const time = entryTime(shown);
  return (
    <FormDialog
      open={entry !== null}
      onOpenChange={onClose}
      title={shown.title}
      description={`${formatDate(time)}, ${formatTime(time)}`}
      footer={<AskMoreButton target={{ kind: "history", entry: shown.id }} noun="History entry" name={shown.id} />}
    >
      <div className="flex flex-col gap-4">
        <div className="font-medium">
          <Markdown holderHeadingLevel={2}>{shown.summary}</Markdown>
        </div>
        {shown.text ? <Markdown holderHeadingLevel={2}>{shown.text}</Markdown> : null}
        <section aria-label="Tasks" className="flex flex-col gap-2">
          <h3 className="text-sm font-semibold">
            {shown.tasks.length} {shown.tasks.length === 1 ? "Task" : "Tasks"}
          </h3>
          <ul className="flex flex-col gap-2">
            {shown.tasks.map((task) => (
              <TaskRow key={task.id} task={task} now={now} />
            ))}
          </ul>
        </section>
      </div>
    </FormDialog>
  );
}

export interface HistoryViewProps {
  /** The current time in milliseconds; tests pass a fixed one. */
  now?: () => number;
}

/**
 * The History of the project window: the entries newest first by their date, grouped by local
 * day, filtered by a period (`Last 7 days` at each opening of the window). What is in an entry
 * and its order come from the backend; this view only groups by day and filters by period,
 * because only the interface knows the local time zone.
 */
export function HistoryView({ now = Date.now }: HistoryViewProps) {
  const { projectId } = useProject();
  const { snapshot, loadError, reload } = useHistory(projectId);
  const [period, setPeriod] = useState<Period>(DEFAULT_PERIOD);
  const [openedId, setOpenedId] = useState<string | null>(null);
  const instant = now();

  // The palette chose an entry: it opens, on `All time` when the chosen period hides it.
  useReveal(
    "history",
    (target) => {
      const entry = snapshot?.entries.find((candidate) => candidate.id === target.id);
      if (!entry) return;
      if (!inPeriod(entryTime(entry), period, instant)) setPeriod("all");
      setOpenedId(entry.id);
    },
    snapshot !== null,
  );

  const periodSelect = (
    <Select value={period} onValueChange={(value) => setPeriod(value as Period)}>
      <SelectTrigger aria-label="Period" size="sm" className="w-44">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {PERIODS.map((choice) => (
          <SelectItem key={choice.id} value={choice.id}>
            {choice.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );

  if (!snapshot) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <ViewHeader view="history" subtitle={SUBTITLE}>
          {periodSelect}
        </ViewHeader>
        {loadError ? (
          <div className="flex flex-col items-start gap-3 p-4">
            <Alert variant="error" role="alert">
              <p className="min-w-0 flex-1">{loadError}</p>
            </Alert>
            <Button variant="outline" onClick={reload}>
              Try again
            </Button>
          </div>
        ) : (
          <p role="status" className="p-4 text-muted-foreground">
            Loading the History…
          </p>
        )}
      </div>
    );
  }

  const groups = groupByDay(snapshot.entries, period, instant);
  const totals = periodTotals(groups, instant);
  const opened = snapshot.entries.find((entry) => entry.id === openedId) ?? null;
  return (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="history" subtitle={SUBTITLE}>
        {periodSelect}
      </ViewHeader>
      <div className="min-h-0 flex-1 overflow-y-auto px-7 pb-10 pt-6">
        <div className="flex w-full max-w-[1040px] flex-col gap-7">
          <dl className="grid grid-cols-[repeat(auto-fit,minmax(180px,1fr))] gap-3">
            <TotalBox label="Delivered" value={String(totals.delivered)} />
            <TotalBox label="Tasks done" value={String(totals.tasksDone)} />
            <TotalBox label="Last delivery" value={totals.lastDelivery ?? "—"} />
          </dl>
          {groups.length === 0 ? (
            <p className="text-muted-foreground">Nothing done in this period</p>
          ) : (
            groups.map((group) => <DayList key={group.day} group={group} onOpen={(entry) => setOpenedId(entry.id)} />)
          )}
        </div>
      </div>
      <EntryDialog entry={opened} now={instant} onClose={(open) => !open && setOpenedId(null)} />
    </div>
  );
}
