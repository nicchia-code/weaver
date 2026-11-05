import { Play, Plus, ScanSearch, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";

import type { BoardCard } from "@/bindings/BoardCard";
import type { Ghost } from "@/bindings/Ghost";
import { BoardUiContext, useBoardActions, type BoardUi } from "@/components/board/board-ui";
import { useBoard } from "@/components/board/board-state";
import { rendererFor } from "@/components/board/card-registry";
import { Column } from "@/components/board/column";
import "@/components/board/default-cards";
import { GhostCard } from "@/components/board/ghost-card";
import { CardId } from "@/components/card-id";
import { ConfirmDialog, FormDialog } from "@/components/dialogs";
import { GuardedButton } from "@/components/guarded-button";
import { MergeBanner } from "@/components/merge/merge-banner";
import { MergeButton } from "@/components/merge/merge-button";
import { useMerge } from "@/components/merge/use-merge";
import { NoticeStack } from "@/components/notices";
import { useViewShown } from "@/components/view-shown";
import { ViewHeader } from "@/components/view-header";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

type ColumnName = "backlog" | "active";

interface OpenCard {
  id: string;
  /** The column the card was in when it was opened. */
  column: ColumnName;
  /** Opened from a row of a resolution: the Drift is not a card of the Board, only its details exist. */
  gathered: boolean;
}

/** Ghosts go to Backlog for a Proposal or a Drift, to Active for a Task. */
function ghostColumn(ghost: Ghost): ColumnName {
  return ghost.kind === "task" ? "active" : "backlog";
}

function CardList({ cards }: { cards: readonly BoardCard[] }) {
  return (
    <ul className="flex flex-col gap-2">
      {cards.map((card) => (
        <li key={card.head.id}>
          <CardOf card={card} />
        </li>
      ))}
    </ul>
  );
}

/** One card through the renderer of its kind. */
function CardOf({ card }: { card: BoardCard }) {
  const { Card } = rendererFor(card);
  return <Card card={card} />;
}

function GhostList({ ghosts }: { ghosts: readonly Ghost[] }) {
  if (ghosts.length === 0) return null;
  return (
    <ul className="flex flex-col gap-2">
      {ghosts.map((ghost) => (
        <li key={ghost.id}>
          <GhostCard ghost={ghost} />
        </li>
      ))}
    </ul>
  );
}

interface DetailsProps {
  card: BoardCard | null;
  /** Where the focus goes when the dialog closes by itself because the card left the Board. */
  focusTarget: RefObject<HTMLElement | null>;
  onClose: () => void;
}

/** The details dialog of a card: the Board's frame around what the card's renderer shows. */
function CardDetails({ card, focusTarget, onClose }: DetailsProps) {
  // The last card shown keeps the dialog's content while it closes.
  const last = useRef<BoardCard | null>(null);
  if (card) last.current = card;
  const shown = card ?? last.current;
  if (!shown) return null;
  const renderer = rendererFor(shown);
  const { Details, Footer } = renderer;
  return (
    <FormDialog
      open={card !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={
        <span className="flex items-baseline gap-2">
          <CardId shortNumber={shown.head.shortNumber} permanentId={shown.head.id} />
          <span>{shown.head.title}</span>
        </span>
      }
      description={renderer.description?.(shown)}
      footer={Footer ? <Footer card={shown} /> : undefined}
      returnFocus={focusTarget}
    >
      <Details card={shown} />
    </FormDialog>
  );
}

/** `Run all` and the other controls of the Board that start work of the agent. */
function BoardFooterButtons({ runAllReasons }: { runAllReasons: readonly string[] }) {
  const actions = useBoardActions();
  return (
    <GuardedButton className="w-full" tooltip="Run all the Tasks in To do" reasons={runAllReasons} onClick={() => actions.runAll?.()}>
      <Play aria-hidden />
      Run all
    </GuardedButton>
  );
}

/**
 * The Board of the project window: `Scan System` in the header, the notices, and the two columns
 * `Backlog` and `Active`. What each column holds, in which order, what is blocked and why a
 * control is disabled all come from the backend's snapshot; this view groups and sorts nothing.
 */
export function BoardView() {
  const { snapshot, loadError, notices, closeNotice, reload } = useBoard();
  const actions = useBoardActions();
  const merge = useMerge();
  const shown = useViewShown();
  const [opened, setOpened] = useState<OpenCard | null>(null);
  const [scanning, setScanning] = useState(false);
  // The History warnings the developer closed, by text: they stay hidden while the window is
  // open, and one whose text changes is a new warning. Nothing of this is written anywhere.
  const [closedWarnings, setClosedWarnings] = useState<ReadonlySet<string>>(() => new Set());
  const backlogRef = useRef<HTMLElement>(null);
  const activeRef = useRef<HTMLElement>(null);
  const focusTarget = useRef<HTMLElement | null>(null);

  // Merge is asked again each time the Board is shown (`merge_refresh`), never on a timer.
  const refreshMerge = useRef(merge?.refresh);
  refreshMerge.current = merge?.refresh;
  useEffect(() => {
    if (shown) refreshMerge.current?.();
  }, [shown]);

  const cards = useMemo(() => {
    const all = new Map<string, BoardCard>();
    for (const card of [...(snapshot?.backlog ?? []), ...(snapshot?.active ?? [])]) all.set(card.head.id, card);
    return all;
  }, [snapshot]);
  const gatheredCards = useMemo(() => new Map((snapshot?.gathered ?? []).map((card) => [card.head.id, card] as const)), [snapshot]);
  const openCard = opened ? (cards.get(opened.id) ?? (opened.gathered ? gatheredCards.get(opened.id) : undefined) ?? null) : null;

  // The card left the Board (decided, done, deleted, gathered into a resolution): the dialog
  // closes by itself and the focus goes to the column the card was in. This runs during the
  // render that closes the dialog, so the target is set before the dialog hands the focus back.
  if (opened && !openCard && snapshot) {
    focusTarget.current = (opened.column === "backlog" ? backlogRef : activeRef).current;
  }
  useEffect(() => {
    if (opened && !openCard && snapshot) setOpened(null);
  }, [opened, openCard, snapshot]);

  const openDetails = useCallback(
    (id: string) => {
      const column: ColumnName = snapshot?.backlog.some((card) => card.head.id === id) ? "backlog" : "active";
      const gathered = !snapshot?.backlog.some((card) => card.head.id === id) && !snapshot?.active.some((card) => card.head.id === id);
      focusTarget.current = null;
      setOpened({ id, column, gathered });
    },
    [snapshot],
  );
  const ui = useMemo<BoardUi>(() => ({ openDetails }), [openDetails]);

  if (!snapshot) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <ViewHeader view="board" />
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
            Loading the Board…
          </p>
        )}
      </div>
    );
  }

  const ghostsOf = (column: ColumnName) => snapshot.ghosts.filter((ghost) => ghostColumn(ghost) === column);
  const visibleWarnings = snapshot.warnings.filter((text) => !closedWarnings.has(text));
  return (
    <BoardUiContext.Provider value={ui}>
      <div className="flex h-full min-h-0 flex-col">
        <ViewHeader view="board">
          <MergeButton />
          <GuardedButton variant="outline" size="sm" reasons={[merge?.merge?.inProgress && "A merge is in progress"]} onClick={() => setScanning(true)}>
            <ScanSearch aria-hidden />
            Scan System
          </GuardedButton>
          {snapshot.doneSinceLastScan > 0 ? (
            <span className="text-muted-foreground">· {snapshot.doneSinceLastScan} done since last scan</span>
          ) : null}
        </ViewHeader>
        <MergeBanner />
        <NoticeStack notices={notices} onClose={closeNotice} />
        {visibleWarnings.length > 0 ? (
          <div className="flex flex-col gap-2 px-4 pt-4">
            {visibleWarnings.map((text) => (
              <Alert key={text} variant="warning">
                <TriangleAlert aria-hidden />
                <p className="min-w-0 flex-1">{text}</p>
                <GuardedButton
                  variant="ghost"
                  size="icon"
                  className="-my-1.5 size-7"
                  label="Close"
                  onClick={() => setClosedWarnings((closed) => new Set(closed).add(text))}
                >
                  <X aria-hidden />
                </GuardedButton>
              </Alert>
            ))}
          </div>
        ) : null}
        <div className="grid min-h-0 flex-1 grid-cols-2 gap-4 p-4">
          <Column
            ref={backlogRef}
            title="Backlog"
            count={snapshot.backlog.length}
            emptyText="Nothing to decide"
            cards={<CardList cards={snapshot.backlog} />}
            ghosts={<GhostList ghosts={ghostsOf("backlog")} />}
            footer={
              <GuardedButton
                variant="outline"
                className="w-full border-dashed"
                tooltip="Write a work item in Markdown and send it to an agent"
                onClick={() => actions.newRequest?.()}
              >
                <Plus aria-hidden />
                Work Item
              </GuardedButton>
            }
          />
          <Column
            ref={activeRef}
            title="Active"
            count={snapshot.active.length}
            emptyText="Nothing active"
            cards={<CardList cards={snapshot.active} />}
            ghosts={<GhostList ghosts={ghostsOf("active")} />}
            footer={<BoardFooterButtons runAllReasons={snapshot.runAllReasons} />}
          />
        </div>
        <CardDetails
          card={openCard}
          focusTarget={focusTarget}
          onClose={() => {
            // Closed by the developer: the focus goes back to what opened it.
            focusTarget.current = null;
            setOpened(null);
          }}
        />
        <ConfirmDialog
          open={scanning}
          onOpenChange={setScanning}
          title="Scan System"
          description="Compare the whole System with the code?"
          confirmLabel="Scan System"
          onConfirm={() => actions.scanSystem?.()}
        />
      </div>
    </BoardUiContext.Provider>
  );
}
