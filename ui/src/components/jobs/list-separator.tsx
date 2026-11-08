import { useRef, type KeyboardEvent, type PointerEvent } from "react";

import { LIST_MAX, LIST_MIN, LIST_STEP, LIST_WIDTH } from "@/components/jobs/use-jobs-list";

/**
 * The rule between the Jobs list and the conversation, as a vertical separator the developer can
 * drag (8 px of grip, `col-resize`) or move with the keyboard: arrows 16 px, `Home` and `End` the
 * narrowest and the widest, `Enter` and a double click the first width. `width` is the chosen
 * width and `shown` the one on screen, where a drag starts. `onResize` gets `commit` false while
 * the pointer is still moving and true when a change is final.
 */
export function ListSeparator({
  width,
  shown,
  onResize,
}: {
  width: number;
  shown: number;
  onResize: (width: number, commit: boolean) => void;
}) {
  const drag = useRef<{ x: number; from: number } | null>(null);
  const at = (event: PointerEvent) => (drag.current ? drag.current.from + event.clientX - drag.current.x : shown);

  const onKeyDown = (event: KeyboardEvent) => {
    const next: Record<string, number> = {
      ArrowLeft: width - LIST_STEP,
      ArrowDown: width - LIST_STEP,
      ArrowRight: width + LIST_STEP,
      ArrowUp: width + LIST_STEP,
      Home: LIST_MIN,
      End: LIST_MAX,
      Enter: LIST_WIDTH,
    };
    if (!(event.key in next)) return;
    event.preventDefault();
    onResize(next[event.key]!, true);
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize jobs list"
      aria-valuemin={LIST_MIN}
      aria-valuemax={LIST_MAX}
      aria-valuenow={width}
      tabIndex={0}
      className="group absolute inset-y-0 -right-1 z-10 w-2 cursor-col-resize touch-none focus-visible:outline-none"
      onKeyDown={onKeyDown}
      onDoubleClick={() => onResize(LIST_WIDTH, true)}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { x: event.clientX, from: shown };
      }}
      onPointerMove={(event) => {
        if (drag.current) onResize(at(event), false);
      }}
      onPointerUp={(event) => {
        if (!drag.current) return;
        onResize(at(event), true);
        drag.current = null;
      }}
      onPointerCancel={(event) => {
        if (!drag.current) return;
        onResize(at(event), true);
        drag.current = null;
      }}
    >
      <span
        aria-hidden
        className="absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 transition-colors group-hover:bg-ring/40 group-focus-visible:bg-ring group-active:bg-ring"
      />
    </div>
  );
}
