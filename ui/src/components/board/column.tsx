import { forwardRef, useId, type ReactNode } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { TONE_DOT, type Tone } from "@/lib/tones";
import { cn } from "@/lib/utils";

export interface ColumnProps {
  title: string;
  /** The thread of the 8 px dot before the title. */
  tone: Tone;
  /** The number of real cards; ghosts never count and a resolution counts once. */
  count: number;
  /** What an empty column shows, also above ghosts. */
  empty: ReactNode;
  /** The cards, already in the order the backend gave. */
  cards: ReactNode;
  /** The ghosts, after the cards. */
  ghosts?: ReactNode;
  /** At the end of the heading (the Backlog's `New Request`). */
  action?: ReactNode;
}

/**
 * One of the two columns of the Board, of equal width: a lane (Lane background, inset 1 px ring,
 * radius 14 px, padding 14 px, 12 px between cards) under its heading, a dot in its thread, the
 * title and the number of real cards in Muted. It scrolls on its own, and takes the focus (it is a
 * focus target, not a tab stop) when a details dialog closes because its card left the Board.
 */
export const Column = forwardRef<HTMLElement, ColumnProps>(function Column({ title, tone, count, empty, cards, ghosts, action }, ref) {
  const headingId = useId();
  return (
    <section
      ref={ref}
      tabIndex={-1}
      aria-labelledby={headingId}
      className="flex min-h-0 min-w-0 flex-col rounded-[14px] bg-lane p-3.5 shadow-[inset_0_0_0_1px_var(--line-soft)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <div className="flex items-center gap-2 px-1 pb-3 pt-0.5">
        <span aria-hidden className={cn("size-2 shrink-0 rounded-full", TONE_DOT[tone])} />
        <h2 className="flex items-baseline gap-2 text-[13px] font-semibold">
          <span id={headingId}>{title}</span>
          <span className="text-xs font-normal text-muted-foreground">{count}</span>
        </h2>
        {action ? <div className="ml-auto">{action}</div> : null}
      </div>
      <ScrollArea className="-mx-1.5 min-h-0 flex-1">
        <div className="flex flex-col gap-3 px-1.5 pb-2">
          {count === 0 ? empty : null}
          {cards}
          {ghosts}
        </div>
      </ScrollArea>
    </section>
  );
});
