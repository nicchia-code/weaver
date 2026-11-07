import { Minus, X } from "lucide-react";
import { useRef, useState, type KeyboardEvent, type ReactNode } from "react";

import type { ActionInput } from "@/bindings/ActionInput";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import { useMediaQuery } from "@/hooks/use-media-query";
import { QuestionIndex } from "@/components/questions/question-index";
import { headerOf, positionOf, waitingOf } from "@/components/questions/question-model";
import { NO_HANDLERS, QuestionPane, type KeyHandlers } from "@/components/questions/question-pane";
import { SummaryPane } from "@/components/questions/summary-pane";
import { QuestionRoundProvider, useQuestionRound } from "@/components/questions/use-question-round";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/** Below this window width the index becomes a row of numbered dots. */
const COMPACT_BELOW = 600;

function isTextEntry(target: EventTarget): boolean {
  return target instanceof HTMLElement && (target.tagName === "TEXTAREA" || (target.tagName === "INPUT" && (target as HTMLInputElement).type === "text"));
}

/** The bar that stays when the overlay is reduced: the agent still waits. */
function MinimizedBar({ round, act }: { round: QuestionRoundView; act: (action: ActionInput) => void }) {
  const where = round.position.kind === "summary" ? "summary" : `question ${round.position.index + 1} of ${round.questions.length}`;
  const who = round.origin.kind === "drift" ? "Weaver" : "The agent";
  return (
    <div
      role="region"
      aria-label="Questions waiting"
      className="fixed inset-x-0 bottom-0 z-40 flex flex-wrap items-center justify-between gap-2 border-t border-peach bg-background px-4 py-2 text-sm text-peach-ink"
    >
      <span className="font-medium">
        {who} is waiting for your answers ({where})
      </span>
      <Button size="sm" onClick={() => act({ kind: "minimize", minimized: false })}>
        Open questions
      </Button>
    </div>
  );
}

/** The modal overlay over the whole window while a round is presented. */
export function QuestionOverlay() {
  const { round, act } = useQuestionRound();
  const compact = useMediaQuery(`(max-width: ${COMPACT_BELOW - 0.02}px)`);
  const [confirmingSkipAll, setConfirmingSkipAll] = useState(false);
  const handlers = useRef<KeyHandlers>(NO_HANDLERS);

  if (!round) return null;
  if (round.minimized) return <MinimizedBar round={round} act={act} />;

  const waiting = waitingOf(round);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    // Keys of the confirmation dialog (a portal) bubble through React but are not ours.
    if (!(event.target instanceof Node) || !event.currentTarget.contains(event.target)) return;
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
      event.preventDefault();
      handlers.current.primary();
      return;
    }
    if (/^[1-9]$/.test(event.key) && !event.ctrlKey && !event.metaKey && !event.altKey && !isTextEntry(event.target)) {
      event.preventDefault();
      handlers.current.digit(Number(event.key) - 1);
    }
  }

  const position = round.position;
  return (
    <>
      {/* Esc and a click outside do nothing: the round leaves only through its own controls. */}
      <Dialog open onOpenChange={() => {}}>
        <DialogContent
          hideClose
          onEscapeKeyDown={(event) => event.preventDefault()}
          onPointerDownOutside={(event) => event.preventDefault()}
          onInteractOutside={(event) => event.preventDefault()}
          onOpenAutoFocus={(event) => event.preventDefault()}
          onKeyDown={onKeyDown}
          className="max-w-[760px] gap-3"
        >
          <DialogHeader className="pr-0">
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 flex-col gap-1">
                <DialogTitle className="truncate text-base">{headerOf(round.origin)}</DialogTitle>
                <DialogDescription>
                  {positionOf(round)}
                  {waiting ? <span className="ml-3">{waiting}</span> : null}
                </DialogDescription>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <Button variant="ghost" size="icon" aria-label="Minimize" title="Minimize" onClick={() => act({ kind: "minimize", minimized: true })}>
                  <Minus aria-hidden />
                </Button>
                <Button variant="ghost" size="icon" aria-label="Close" title="Close" onClick={() => setConfirmingSkipAll(true)}>
                  <X aria-hidden />
                </Button>
              </div>
            </div>
          </DialogHeader>
          <div className={compact ? "flex min-h-0 flex-1 flex-col gap-3" : "flex min-h-0 flex-1 gap-4"}>
            <QuestionIndex round={round} compact={compact} go={(next) => act({ kind: "go", position: next })} />
            <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-4">
              {position.kind === "summary" ? (
                <SummaryPane key={`${round.id}:summary`} round={round} act={act} askSkipAll={() => setConfirmingSkipAll(true)} handlers={handlers} />
              ) : (
                <QuestionPane
                  key={`${round.id}:${position.index}`}
                  round={round}
                  index={position.index}
                  act={act}
                  askSkipAll={() => setConfirmingSkipAll(true)}
                  handlers={handlers}
                />
              )}
            </div>
          </div>
        </DialogContent>
      </Dialog>
      <AlertDialog open={confirmingSkipAll} onOpenChange={setConfirmingSkipAll}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Skip all unanswered questions?</AlertDialogTitle>
            <AlertDialogDescription>
              The agent will use its recommendation for each of them.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Go back</AlertDialogCancel>
            <AlertDialogAction onClick={() => act({ kind: "skip-all" })}>Skip all</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

/** Gives a project window the round the backend presents in it, as a modal overlay above `children`. */
export function QuestionRoundLayer({ projectId, children }: { projectId: string; children: ReactNode }) {
  return (
    <QuestionRoundProvider projectId={projectId}>
      {children}
      <QuestionOverlay />
    </QuestionRoundProvider>
  );
}
