import type { MouseEvent, ReactNode } from "react";

import { useBoardUi } from "@/components/board/board-ui";
import { CardId } from "@/components/card-id";
import { cn } from "@/lib/utils";

export interface CardShellProps {
  /** The permanent identity: names the card to the Board (details, focus). */
  cardId: string;
  /** `T4`, when the card has a number. */
  shortNumber: string | null;
  /** A label in the place of the id (`Request`, on the draft of a request, which has no number). */
  tag?: string;
  title: string;
  /** What follows the id in Muted (the label of a Task). */
  meta?: ReactNode;
  /** Pushed to the right end of the id row (the status pill of a Task). */
  status?: ReactNode;
  /** The kind mark, at the right of the title. */
  mark?: ReactNode;
  /** Plain text under the title (the first paragraph of a Task's body). */
  summary?: ReactNode;
  /** Class of the card's own accent (a left stripe in a thread, a tint, the rotating border of a held Task). */
  accent?: string;
  /** The badges or chips, in their order. */
  badges?: ReactNode;
  /** What sits between the badges and the footer. */
  body?: ReactNode;
  /** The footer after a rule: the controls. A click on them never opens the details. */
  controls?: ReactNode;
  /** What a click on the card outside its controls, or Enter on its title, does; by default it opens the card's details. */
  onOpen?: () => void;
}

const OTHER_CONTROLS = "button, a, input, select, textarea, [role='button'], [role='menuitem']";

/**
 * The frame every card of the Board shares (`requirements/ui#task`): Surface with a 1 px Line ring
 * and the soft shadow of a card on a lane, radius 12 px, 12 px between its rows: the id tag with
 * what follows it and the status at the right, the title (15 px semibold, at most 3 lines, then
 * `…`) with the kind mark and the summary, the badges, a body, and the controls in a footer after a
 * rule. A click on the card outside its controls, or Enter on its title, opens the details dialog;
 * no card can be dragged, moved or removed.
 */
export function CardShell({ cardId, shortNumber, tag, title, meta, status, mark, summary, accent, badges, body, controls, onOpen }: CardShellProps) {
  const { openDetails } = useBoardUi();
  const open = (event: MouseEvent<HTMLDivElement>) => {
    const target = event.target instanceof Element ? event.target : null;
    const control = target?.closest(OTHER_CONTROLS);
    // The title is the one button that opens the details; every other control keeps its click.
    if (control && !control.hasAttribute("data-card-title")) return;
    if (target?.closest("[data-card-controls]")) return;
    if (onOpen) onOpen();
    else openDetails(cardId);
  };
  return (
    // Keyboard users open the details with Enter on the title (a button); the click handler of
    // the frame serves the pointer, and the title's click reaches it too. The 2 px transparent
    // border is where a held Task draws its rotating one, so no card moves when one is held.
    <div
      data-card-id={cardId}
      onClick={open}
      className={cn(
        "flex cursor-pointer flex-col gap-3 rounded-xl border-2 border-transparent bg-card px-3.5 pb-3.5 pt-3 text-card-foreground shadow-card transition-colors duration-150 hover:bg-accent/40",
        accent,
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        {tag ? (
          <span className="inline-block shrink-0 rounded-sm bg-paper px-1.5 py-0.5 font-mono text-xs">{tag}</span>
        ) : (
          <CardId shortNumber={shortNumber} permanentId={cardId} className="shrink-0" />
        )}
        {meta}
        {status ? <div className="ml-auto shrink-0">{status}</div> : null}
      </div>
      <div className="flex flex-col gap-1.5">
        <div className="flex items-start gap-2">
          <h3 className="min-w-0 flex-1 text-[15px] font-semibold leading-snug tracking-tight">
            <button
              type="button"
              data-card-title
              className="line-clamp-3 w-full rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {title}
            </button>
          </h3>
          {mark ? <div className="shrink-0">{mark}</div> : null}
        </div>
        {summary}
      </div>
      {badges ? <div className="flex flex-wrap items-center gap-1.5">{badges}</div> : null}
      {body}
      {controls ? (
        <div data-card-controls className="flex flex-wrap items-center gap-2 border-t border-line-soft pt-3">
          {controls}
        </div>
      ) : null}
    </div>
  );
}
