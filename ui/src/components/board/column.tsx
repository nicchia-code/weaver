import { forwardRef, useEffect, useId, useState, type ReactNode } from "react";

import { FadeIn } from "@/components/board/motion-list";
import { ScrollArea } from "@/components/ui/scroll-area";
import { TONE_DOT, type Tone } from "@/lib/tones";
import { cn } from "@/lib/utils";

export interface ColumnProps {
  title: string;
  /** The thread of the 8 px dot before the title. */
  tone: Tone;
  /** The number of real cards; ghosts never count and a resolution counts once. */
  count: number;
  /** What an empty column shows: only with no real card and no ghost. */
  empty: ReactNode;
  /** The number of ghosts; they hide the empty state like a card but never count. */
  ghostCount: number;
  /** The cards and the ghosts, already in the order the backend gave (`MotionList`). */
  children?: ReactNode;
}

/**
 * One of the two columns of the Board, of equal width: a lane (Lane background, inset 1 px ring,
 * radius 14 px, padding 14 px, 12 px between cards) under its heading, a dot in its thread, the
 * title and the number of real cards in Muted. It scrolls on its own, and takes the focus (it is a
 * focus target, not a tab stop) when a details dialog closes because its card left the Board.
 */
export const Column = forwardRef<HTMLElement, ColumnProps>(function Column({ title, tone, count, empty, ghostCount, children }, ref) {
  const headingId = useId();
  // The empty state that is there when the column first shows does not fade; one that comes back
  // after the last card or ghost left fades in while that one fades out.
  const [shown, setShown] = useState(false);
  useEffect(() => setShown(true), []);
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
      </div>
      <ScrollArea className="-mx-1.5 min-h-0 flex-1">
        <div className="flex flex-col gap-3 px-1.5 pb-2">
          {count === 0 && ghostCount === 0 ? <FadeIn animate={shown}>{empty}</FadeIn> : null}
          {children}
        </div>
      </ScrollArea>
    </section>
  );
});
