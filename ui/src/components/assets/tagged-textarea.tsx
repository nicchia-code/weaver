import * as React from "react";

import { iconOf } from "@/components/assets/asset-kinds";
import { BOX_METRICS, refuseMetricStyles, refuseMetrics } from "@/components/assets/box-metrics";
import { findTags, insertTag, suggestionsFor, typingAt, type Tag } from "@/components/assets/tags";
import { useAssets } from "@/components/assets/use-assets";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

/** The pieces the mirror draws: the text cut at the tags (drawn in the primary blue) and, when `caret` is given, at the caret. */
type Piece = { kind: "text"; text: string; tag: Tag | null } | { kind: "caret" };

function piecesOf(value: string, tags: readonly Tag[], caret: number | null): Piece[] {
  const segments: { from: number; to: number; tag: Tag | null }[] = [];
  let at = 0;
  for (const tag of tags) {
    if (tag.start > at) segments.push({ from: at, to: tag.start, tag: null });
    segments.push({ from: tag.start, to: tag.end, tag });
    at = tag.end;
  }
  if (at < value.length) segments.push({ from: at, to: value.length, tag: null });

  const caretAt = caret === null ? null : Math.max(0, Math.min(caret, value.length));
  const pieces: Piece[] = [];
  let placed = caretAt === null;
  const push = (from: number, to: number, tag: Tag | null) => {
    if (to > from) pieces.push({ kind: "text", text: value.slice(from, to), tag });
  };
  for (const segment of segments) {
    if (!placed && caretAt !== null && caretAt >= segment.from && caretAt <= segment.to) {
      push(segment.from, caretAt, segment.tag);
      pieces.push({ kind: "caret" });
      push(caretAt, segment.to, segment.tag);
      placed = true;
    } else {
      push(segment.from, segment.to, segment.tag);
    }
  }
  if (!placed) pieces.push({ kind: "caret" });
  return pieces;
}

/** Sets the value of a textarea the way typing does, so that React's `onChange` of the owner hears of it. */
function typeInto(area: HTMLTextAreaElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set?.call(area, text);
  area.dispatchEvent(new Event("input", { bubbles: true }));
}

export interface TaggedTextareaProps extends Omit<React.ComponentProps<"textarea">, "value" | "role"> {
  value: string;
  /** Classes of the box itself: its size and its place in the layout. Never a metric of the text. */
  className?: string;
  /**
   * The metrics of the text: padding, border width, font family, size, line height, letter spacing, wrapping.
   * They reach the textarea and the mirror alike, and this is the only place to change them.
   */
  metricsClassName?: string;
  /** Classes of the native `textarea` alone: background, border colour, focus ring, resize. Never a metric of the text. */
  areaClassName?: string;
}

/**
 * A message box that recognises the `@` tags of the project's assets (`requirements/ui#assets`).
 * It serves the Chat box, the reply box of a job, the box of an Ask more conversation and the
 * text of the request editor.
 *
 * The native `textarea` keeps the typing, the caret, the selection, the undo and the accessibility;
 * its own text is transparent and its caret stays visible. Behind it a mirror layer draws the text,
 * the tags recognised against the assets of the window in the primary blue. Typing `@` opens a list
 * of the assets under the caret.
 *
 * The metrics of the text (padding, border width, font, size, line height, letter spacing, wrapping)
 * are set once here, with `metricsClassName`, and reach both layers; `className` and `areaClassName`
 * refuse a metric, since one layer alone would draw the characters away from the caret.
 */
export const TaggedTextarea = React.forwardRef<HTMLTextAreaElement, TaggedTextareaProps>(function TaggedTextarea(
  { value, className, metricsClassName, areaClassName, style, disabled, onChange, onKeyDown, onKeyUp, onSelect, onClick, onFocus, onBlur, onScroll, ...props },
  ref,
) {
  refuseMetrics("className", className);
  refuseMetrics("areaClassName", areaClassName);
  refuseMetricStyles(style);
  const metrics = cn(BOX_METRICS, metricsClassName);
  const { assets } = useAssets();
  const names = React.useMemo(() => assets.map((asset) => asset.name), [assets]);
  const listId = React.useId();
  const area = React.useRef<HTMLTextAreaElement | null>(null);
  const mirror = React.useRef<HTMLDivElement | null>(null);
  const marker = React.useRef<HTMLSpanElement | null>(null);
  const [caret, setCaret] = React.useState(0);
  const [focused, setFocused] = React.useState(false);
  const [dismissed, setDismissed] = React.useState(false);
  const [active, setActive] = React.useState(0);
  const [anchor, setAnchor] = React.useState({ left: 0, top: 0 });
  /** Where the caret goes once the text of a chosen asset is in. */
  const pending = React.useRef<number | null>(null);

  const tags = React.useMemo(() => findTags(value, names), [value, names]);
  const typing = focused && !disabled && !dismissed ? typingAt(value, caret) : null;
  const query = typing?.query ?? null;
  const shown = React.useMemo(() => (query === null ? [] : suggestionsFor(names, query)), [names, query]);
  const open = typing !== null && shown.length > 0;
  const current = Math.min(active, shown.length - 1);
  const optionId = (index: number) => `${listId}-${index}`;

  const syncScroll = () => {
    if (area.current && mirror.current) {
      mirror.current.scrollTop = area.current.scrollTop;
      mirror.current.scrollLeft = area.current.scrollLeft;
    }
  };

  // The caret goes after the asset that was chosen, once React has put the new text in the box.
  React.useLayoutEffect(() => {
    if (pending.current !== null && area.current) {
      const to = pending.current;
      pending.current = null;
      area.current.setSelectionRange(to, to);
      setCaret(to);
    }
    syncScroll();
  }, [value]);

  // The list opens under the caret, which is measured on the mirror.
  React.useLayoutEffect(() => {
    if (!open || !marker.current || !mirror.current) return;
    const left = marker.current.offsetLeft - mirror.current.scrollLeft;
    const top = marker.current.offsetTop + marker.current.offsetHeight - mirror.current.scrollTop;
    setAnchor((now) => (now.left === left && now.top === top ? now : { left, top }));
  }, [open, value, caret, tags]);

  const choose = (name: string) => {
    const box = area.current;
    if (!box || !typing) return;
    const next = insertTag(value, box.selectionStart ?? caret, typing, name);
    pending.current = next.caret;
    typeInto(box, next.text);
  };

  const keyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter or Tab that confirm a composition of the input method only confirm it: the box does nothing with them.
    // Enter is left to the input method untouched; Tab only has its default action, a move of the focus, cancelled.
    if ((event.key === "Enter" || event.key === "Tab") && (event.nativeEvent.isComposing || event.keyCode === 229)) {
      if (event.key === "Tab") event.preventDefault();
      return;
    }
    if (open) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const step = event.key === "ArrowDown" ? 1 : -1;
        setActive((current + step + shown.length) % shown.length);
        return;
      }
      if ((event.key === "Enter" && !event.shiftKey) || (event.key === "Tab" && !event.shiftKey)) {
        // With the list open neither key sends the message nor moves the focus.
        event.preventDefault();
        const name = shown[current];
        if (name !== undefined) choose(name);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setDismissed(true);
        return;
      }
    }
    onKeyDown?.(event);
  };

  const follow = (box: HTMLTextAreaElement) => setCaret(box.selectionStart ?? 0);

  const pieces = piecesOf(value, tags, open ? caret : null);

  return (
    <div className={cn("relative rounded-md bg-background", disabled && "opacity-50", className)}>
      <div
        ref={mirror}
        aria-hidden
        data-testid="tag-mirror"
        className={cn(
          metrics,
          "pointer-events-none absolute inset-0 overflow-auto rounded-md border-transparent text-foreground [scrollbar-color:transparent_transparent]",
        )}
      >
        {pieces.map((piece, index) =>
          piece.kind === "caret" ? (
            // Empty and inline: it adds neither width nor a place to break the line.
            <span key={index} ref={marker} data-testid="tag-caret" />
          ) : piece.tag ? (
            <span key={index} data-tag={piece.tag.name} className="text-sky-ink">
              {piece.text}
            </span>
          ) : (
            <React.Fragment key={index}>{piece.text}</React.Fragment>
          ),
        )}
        {/* A final line break takes room only when something follows it. */}
        {"\u200b"}
      </div>
      <Popover open={open}>
        <PopoverAnchor asChild>
          <span aria-hidden className="pointer-events-none absolute size-0" style={{ left: anchor.left, top: anchor.top }} />
        </PopoverAnchor>
        <textarea
          {...props}
          ref={(node) => {
            area.current = node;
            if (typeof ref === "function") ref(node);
            else if (ref) ref.current = node;
          }}
          value={value}
          disabled={disabled}
          role="combobox"
          autoComplete="off"
          aria-haspopup="listbox"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          aria-activedescendant={open ? optionId(current) : undefined}
          className={cn(
            metrics,
            "relative block h-full w-full rounded-md border-input bg-transparent text-transparent caret-foreground selection:bg-primary/30 selection:text-transparent placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed",
            areaClassName,
          )}
          style={style}
          onChange={(event) => {
            setDismissed(false);
            setActive(0);
            follow(event.target);
            onChange?.(event);
          }}
          onKeyDown={keyDown}
          onKeyUp={(event) => {
            follow(event.currentTarget);
            onKeyUp?.(event);
          }}
          onSelect={(event) => {
            follow(event.currentTarget);
            onSelect?.(event);
          }}
          onClick={(event) => {
            follow(event.currentTarget);
            onClick?.(event);
          }}
          onFocus={(event) => {
            setFocused(true);
            follow(event.currentTarget);
            onFocus?.(event);
          }}
          onBlur={(event) => {
            setFocused(false);
            onBlur?.(event);
          }}
          onScroll={(event) => {
            syncScroll();
            onScroll?.(event);
          }}
        />
        <PopoverContent
          align="start"
          sideOffset={4}
          className="w-auto min-w-48 max-w-96 p-1"
          onOpenAutoFocus={(event) => event.preventDefault()}
          onCloseAutoFocus={(event) => event.preventDefault()}
        >
          <div role="listbox" id={listId} aria-label="Assets" className="max-h-64 overflow-y-auto">
            {shown.map((name, index) => {
              const Icon = iconOf(name);
              return (
                <div
                  key={name}
                  id={optionId(index)}
                  role="option"
                  aria-selected={index === current}
                  // Keep the focus in the box: a blur would close the list before the choice.
                  onMouseDown={(event) => event.preventDefault()}
                  onMouseMove={() => setActive(index)}
                  onClick={() => choose(name)}
                  className={cn(
                    "flex h-8 cursor-default select-none items-center gap-2 rounded-sm px-2 text-sm",
                    index === current ? "bg-accent text-accent-foreground" : "hover:bg-accent hover:text-accent-foreground",
                  )}
                >
                  <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
                  <span className="truncate">{name}</span>
                </div>
              );
            })}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
});
