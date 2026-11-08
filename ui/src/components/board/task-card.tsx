import { Bot, ChevronRight, ListChecks, Package, Play, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { JobRow } from "@/bindings/JobRow";
import type { TitledRef } from "@/bindings/TitledRef";
import { AskMoreButton } from "@/components/ask/ask-button";
import { useBoard } from "@/components/board/board-state";
import { useBoardActions, type FocusView } from "@/components/board/board-ui";
import { registerCardRenderer, type BoardCardOf } from "@/components/board/card-registry";
import { CardShell } from "@/components/board/card-shell";
import { fileChips, type FileChip } from "@/components/board/file-chips";
import { ConfirmDialog } from "@/components/dialogs";
import { cardReasons, GuardedButton } from "@/components/guarded-button";
import { useOptionalJobs } from "@/components/jobs/use-jobs";
import { Markdown } from "@/components/markdown";
import { useProject } from "@/components/project-context";
import { Badge, badgeVariants } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { FocusSubject } from "@/components/views/focus-model";
import { useOptionalViews } from "@/components/views/views-state";
import { formatRunning } from "@/lib/format";
import { firstParagraph } from "@/lib/plain-text";
import { TONE_DOT, TONE_PILL } from "@/lib/tones";
import { cn } from "@/lib/utils";

type TaskCardData = BoardCardOf<"task">;

/** How a card is named in a sentence: its short number, else its permanent id. */
function nameOf(ref: { id: string; shortNumber: string | null }): string {
  return ref.shortNumber ?? ref.id;
}

/** `To do`, `In progress`, or `Agent working` while a job holds the Task. */
export function taskStatus(card: Pick<TaskCardData, "doing" | "heldBy">): string {
  if (card.heldBy !== null) return "Agent working";
  return card.doing ? "In progress" : "To do";
}

/** The running time of a job, ticking every second while it has a session (`null` before). */
function useJobRunning(job: JobRow | undefined): string | null {
  const started = job?.started ?? null;
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (started === null) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [started]);
  return started === null ? null : formatRunning(now - Date.parse(started));
}

/**
 * The status of a Task as a pill, never animated, with its tooltip and accessible name: To do is a
 * neutral pill with a hollow ring, In progress a Sky pill with a filled dot, and while a job holds
 * the Task `Agent working · <running time of that job>`.
 */
function StatusPill({ card, running }: { card: TaskCardData; running: string | null }) {
  const active = card.doing || card.heldBy !== null;
  const label = card.heldBy !== null && running !== null ? `${taskStatus(card)} · ${running}` : taskStatus(card);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          role="img"
          aria-label={label}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium tabular-nums",
            active ? TONE_PILL.sky : "bg-muted text-ink-2",
          )}
        >
          <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", active ? TONE_DOT.sky : "border border-current")} />
          <span aria-hidden>{label}</span>
        </span>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

const blockedText = (refs: readonly TitledRef[]) => `Blocked by Drift ${refs.map(nameOf).join(", ")}`;
const waitingText = (refs: readonly TitledRef[]) => `Waiting for Task ${refs.map(nameOf).join(", ")}`;

/**
 * Scrolls the card (or its row in a resolution card) into view and focuses it. A Drift sits in
 * Backlog as a card of its own or as a row of a resolution; a fix Task sits in Active.
 */
export function revealCard(id: string): void {
  const target =
    document.querySelector<HTMLElement>(`[data-card-id="${id}"]`) ?? document.querySelector<HTMLElement>(`[data-drift-row="${id}"]`);
  if (!target) return;
  target.scrollIntoView?.({ block: "nearest" });
  (target.querySelector<HTMLElement>("[data-card-title]") ?? target).focus();
}

interface RefsPopoverProps {
  /** `blocked` or `waiting`. */
  kind: "blocked" | "waiting";
  refs: readonly TitledRef[];
}

/**
 * The `blocked` and `waiting <id>` badges: a button that opens a Popover listing what blocks the
 * Task (`Blocked by`: Drifts, `Waiting for`: the fix Task). A row closes the popover, scrolls its
 * card into view and focuses it.
 */
function RefsPopover({ kind, refs }: RefsPopoverProps) {
  const [open, setOpen] = useState(false);
  const chosen = useRef<string | null>(null);
  const blocked = kind === "blocked";
  const text = blocked ? blockedText(refs) : waitingText(refs);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={text}
              className={cn(
                badgeVariants({ variant: "warning" }),
                "cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              )}
            >
              {blocked ? "blocked" : `waiting ${refs.map(nameOf).join(", ")}`}
            </button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>{text}</TooltipContent>
      </Tooltip>
      <PopoverContent
        aria-label={blocked ? "Blocked by" : "Waiting for"}
        onCloseAutoFocus={(event) => {
          // The focus goes to the card the developer chose, not back to the badge.
          const id = chosen.current;
          chosen.current = null;
          if (id) {
            event.preventDefault();
            revealCard(id);
          }
        }}
      >
        <p className="mb-2 text-xs font-semibold">{blocked ? "Blocked by" : "Waiting for"}</p>
        <ul className="space-y-1">
          {refs.map((ref) => (
            <li key={ref.id}>
              <button
                type="button"
                className="flex w-full items-baseline gap-2 rounded-sm px-1 py-0.5 text-left text-xs hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onClick={() => {
                  chosen.current = ref.id;
                  setOpen(false);
                }}
              >
                <span className="font-mono">{nameOf(ref)}</span>
                <span className="min-w-0 flex-1">{ref.title}</span>
              </button>
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  );
}

/** The label of a Proposal: text in an outline badge, with no colour of its own. */
export function LabelBadge({ label }: { label: string }) {
  return (
    <Badge variant="outline" title={`Label ${label}`}>
      {label}
    </Badge>
  );
}

/** The chip of a Requirements fragment or of a Specs field a Task names: a button that opens the single-ref focus of the first reference it stands for. */
function FileChipButton({ chip }: { chip: FileChip }) {
  const { viewOn } = useBoardActions();
  return (
    <button
      type="button"
      className={cn(badgeVariants({ variant: "outline" }), "cursor-pointer rounded-sm font-mono text-[11.5px] font-normal hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring")}
      aria-label={`${FOCUS_BUTTONS[chip.view].label}: ${chip.label}`}
      onClick={() => viewOn?.(chip.view, [chip.ref])}
    >
      {chip.label}
    </button>
  );
}

function TaskCard({ card }: { card: TaskCardData }) {
  const { head } = card;
  const { openJob } = useProject();
  const jobs = useOptionalJobs();
  const data = useOptionalViews()?.snapshot?.data;
  const job = card.heldBy === null ? undefined : jobs?.jobs.find((candidate) => candidate.id === card.heldBy);
  const running = useJobRunning(job);
  const summary = firstParagraph(head.body);
  const chips = useMemo(() => fileChips(card.refs, data), [card.refs, data]);
  const subject: FocusSubject = { kind: "task", id: head.id, number: head.shortNumber, title: head.title };
  const held = card.heldBy;
  return (
    <CardShell
      cardId={head.id}
      shortNumber={head.shortNumber}
      title={head.title}
      meta={card.label ? <span className="min-w-0 truncate text-xs text-muted-foreground">{card.label}</span> : null}
      status={<StatusPill card={card} running={running} />}
      summary={summary ? <p className="line-clamp-3 text-[13px] leading-normal text-ink-2">{summary}</p> : null}
      accent={held !== null ? "task-held" : undefined}
      badges={
        card.technical || card.blockedBy.length > 0 || card.waitingFor.length > 0 || chips.length > 0 ? (
          <>
            {card.technical ? (
              <Badge variant="outline" className="rounded-sm font-normal">
                technical
              </Badge>
            ) : null}
            {card.blockedBy.length > 0 ? <RefsPopover kind="blocked" refs={card.blockedBy} /> : null}
            {card.waitingFor.length > 0 ? <RefsPopover kind="waiting" refs={card.waitingFor} /> : null}
            {chips.map((chip) => (
              <FileChipButton key={chip.key} chip={chip} />
            ))}
          </>
        ) : null
      }
      controls={
        <>
          {held !== null ? (
            <>
              <span aria-hidden className={cn("grid size-[22px] shrink-0 place-items-center rounded-md", TONE_PILL.sky)}>
                <Bot className="size-3.5" />
              </span>
              <span className="min-w-0 flex-1 truncate text-xs text-ink-2">{job?.name ?? "Agent"}</span>
            </>
          ) : null}
          <FocusButton view="requirements" refs={card.refs} subject={subject} />
          {held !== null ? (
            <Button variant="secondary" size="sm" onClick={() => openJob(held)}>
              Open job
              <ChevronRight aria-hidden />
            </Button>
          ) : (
            <RunButton card={card} />
          )}
        </>
      }
    />
  );
}

function RunButton({ card }: { card: TaskCardData }) {
  const actions = useBoardActions();
  return (
    <GuardedButton variant="secondary" size="sm" reasons={card.runReasons} onClick={() => actions.runTask?.(card.head.id)}>
      <Play aria-hidden />
      Run
    </GuardedButton>
  );
}

/** `View requirements` (only when its focus has at least one step) and `Run`. */
function TaskControls({ card }: { card: TaskCardData }) {
  const subject: FocusSubject = { kind: "task", id: card.head.id, number: card.head.shortNumber, title: card.head.title };
  return (
    <>
      <FocusButton view="requirements" refs={card.refs} subject={subject} />
      <RunButton card={card} />
    </>
  );
}

const FOCUS_BUTTONS: Record<FocusView, { label: string; icon: typeof ListChecks }> = {
  requirements: { label: "View requirements", icon: ListChecks },
  specs: { label: "View specs", icon: Package },
};

/** Only when the focus on `refs` has at least one step in `view`; `subject` names the Task or Proposal it is about. */
export function FocusButton({ view, refs, subject }: { view: FocusView; refs: readonly string[]; subject?: FocusSubject }) {
  const { focusSteps, viewOn } = useBoardActions();
  if (refs.length === 0 || !viewOn || !focusSteps || focusSteps(view, refs, subject) < 1) return null;
  const { label, icon: Icon } = FOCUS_BUTTONS[view];
  return (
    <GuardedButton variant="ghost" size="sm" onClick={() => viewOn(view, refs, subject)}>
      <Icon aria-hidden />
      {label}
    </GuardedButton>
  );
}

/** A reference as a chip: it focuses the first of Requirements and Specs where that reference has a step. */
export function RefChip({ reference }: { reference: string }) {
  const { focusSteps, viewOn } = useBoardActions();
  const single = [reference];
  const view: FocusView | null =
    viewOn && focusSteps ? (["requirements", "specs"] as const).find((v) => focusSteps(v, single) >= 1) ?? null : null;
  const chip = cn(badgeVariants({ variant: "outline" }), "font-mono font-normal");
  if (!view) return <span className={chip}>{reference}</span>;
  return (
    <button
      type="button"
      className={cn(chip, "cursor-pointer hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring")}
      aria-label={`${FOCUS_BUTTONS[view].label}: ${reference}`}
      onClick={() => viewOn?.(view, single)}
    >
      {reference}
    </button>
  );
}

/** The body of the details dialog: status, badges, label, references, and the text of the Task. */
function TaskDetails({ card }: { card: TaskCardData }) {
  const { head } = card;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={card.doing || card.heldBy !== null ? "info" : "secondary"}>{taskStatus(card)}</Badge>
        {card.technical ? <Badge variant="outline">technical</Badge> : null}
        {card.blockedBy.length > 0 ? <Badge variant="warning">blocked</Badge> : null}
        {card.waitingFor.length > 0 ? <Badge variant="warning">waiting {card.waitingFor.map(nameOf).join(", ")}</Badge> : null}
        {card.label ? <LabelBadge label={card.label} /> : null}
      </div>
      {card.blockedBy.length > 0 ? (
        <p className="text-xs text-muted-foreground">
          {blockedText(card.blockedBy)}: {card.blockedBy.map((ref) => `${nameOf(ref)} ${ref.title}`).join("; ")}
        </p>
      ) : null}
      {card.waitingFor.length > 0 ? (
        <p className="text-xs text-muted-foreground">
          {waitingText(card.waitingFor)}: {card.waitingFor.map((ref) => `${nameOf(ref)} ${ref.title}`).join("; ")}
        </p>
      ) : null}
      {card.refs.length > 0 ? (
        <ul aria-label="References" className="flex flex-wrap gap-1">
          {card.refs.map((reference) => (
            <li key={reference}>
              <RefChip reference={reference} />
            </li>
          ))}
        </ul>
      ) : null}
      {head.body.trim() ? <Markdown holderHeadingLevel={2}>{head.body}</Markdown> : <p className="text-muted-foreground">No description.</p>}
    </div>
  );
}

/** `View requirements`, `Run`, `Ask more` and, only in To do, `Delete` (confirmed). */
function TaskFooter({ card }: { card: TaskCardData }) {
  const { deleteTask } = useBoard();
  const [confirming, setConfirming] = useState(false);
  const name = nameOf(card.head);
  return (
    <>
      <TaskControls card={card} />
      <AskMoreButton target={{ kind: "task", id: card.head.id }} noun="Task" name={card.head.shortNumber ?? card.head.id} />
      {card.doing ? null : (
        <>
          <GuardedButton
            variant="destructive"
            size="sm"
            reasons={cardReasons(card.head.agentFinishing, card.deleteReasons)}
            onClick={() => setConfirming(true)}
          >
            <Trash2 aria-hidden />
            Delete
          </GuardedButton>
          <ConfirmDialog
            open={confirming}
            onOpenChange={setConfirming}
            title={`Delete ${name} for good?`}
            description="The Task is removed and nothing remains of it."
            confirmLabel="Delete"
            destructive
            onConfirm={() => void deleteTask(card.head.id, name)}
          />
        </>
      )}
    </>
  );
}

registerCardRenderer("task", {
  Card: TaskCard,
  Details: TaskDetails,
  Footer: TaskFooter,
  description: (card) => taskStatus(card),
});
