import { ArrowDown, ArrowUp, Square } from "lucide-react";
import { Fragment, useLayoutEffect, useMemo, useRef, type ReactNode, type RefObject } from "react";

import type { Entry } from "@/bindings/Entry";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { SenderRow, WorkSummary } from "@/components/general/chat-conversation";
import { chatItems, latestStep } from "@/components/general/chat-model";
import { EntryView } from "@/components/jobs/entry-view";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useFollowScroll } from "@/hooks/use-follow-scroll";
import { cn } from "@/lib/utils";

/** The column of a conversation of the Jobs view: centred, 860 px at most, 22 px 28 px of padding. */
const COLUMN = "mx-auto flex w-full max-w-[860px] flex-col gap-[18px] px-7 py-[22px]";

/** Follows new text unless the developer scrolled up; then `Jump to latest` shows. `version` changes with the content. */
export function FollowScroller({ version, children }: { version: unknown; children: ReactNode }) {
  const { scrollerRef, contentRef, following, follow, onScroll } = useFollowScroll(version);
  return (
    <div ref={scrollerRef} className="relative min-h-0 flex-1 overflow-y-auto" onScroll={onScroll}>
      <div ref={contentRef} className={COLUMN}>
        {children}
      </div>
      {!following && (
        <Button className="sticky bottom-2 ml-auto mr-4 mt-3 flex" size="sm" variant="secondary" onClick={follow}>
          <ArrowDown aria-hidden />
          Jump to latest
        </Button>
      )}
    </div>
  );
}

/**
 * The entries of a conversation as Chat shows them: the messages of the developer, the messages of
 * the agent under a sender row, and every run of `Thinking` blocks and tool rows as one summary.
 * `working` says a turn is in progress, so that the run that ends the conversation reads `Working`.
 * `agent` names a sub-agent, whose sender row shows its name instead of `Weaver`. `after` adds
 * what follows an entry (a line of Chat, the `Starting Weaver…` line).
 */
export function ConversationEntries({
  entries,
  arrivals,
  working,
  answer,
  agent,
  latest = false,
  after,
}: {
  entries: readonly Entry[];
  /** The instant each entry arrived; shorter than `entries` when they are not known. */
  arrivals: readonly number[];
  working: boolean;
  answer: (request: number, option: string) => void;
  agent?: string;
  /** The running summary names the latest step (`Working · 3 steps · Read the Model`). */
  latest?: boolean;
  after?: (entry: Entry, index: number) => ReactNode;
}) {
  const items = useMemo(() => chatItems(entries, arrivals), [entries, arrivals]);
  return (
    <>
      {items.map((item) => {
        if (item.kind === "work") {
          const running = working && item.to === entries.length - 1;
          return (
            <Fragment key={`work-${item.from}`}>
              {item.starts && <SenderRow at={item.sender} agent={agent} />}
              <WorkSummary
                item={item}
                entries={entries}
                running={running}
                latest={latest && running ? latestStep(item, entries) : null}
                answer={answer}
              />
            </Fragment>
          );
        }
        const entry = entries[item.index]!;
        return (
          <Fragment key={`entry-${item.index}`}>
            {item.starts && <SenderRow at={item.sender} agent={agent} />}
            <EntryView entry={entry} answer={answer} />
            {after?.(entry, item.index)}
          </Fragment>
        );
      })}
    </>
  );
}

/** The width of the message box's column: Chat's 760 px, the 860 px of the Jobs view. */
const FRAMES = {
  chat: { band: "px-6", box: "max-w-[760px]" },
  jobs: { band: "px-7", box: "max-w-[860px]" },
} as const;

const FADE = "bg-gradient-to-b from-transparent to-surface to-[28%] pb-6 pt-5";

/**
 * The message box over its fade: a Surface box with a ring, a 16 px radius and the text over the
 * buttons (`controls`), centred in its column.
 */
export function ComposerFrame({ column, controls, children }: { column: keyof typeof FRAMES; controls: ReactNode; children: ReactNode }) {
  const { band, box } = FRAMES[column];
  return (
    <div className={cn("flex justify-center", FADE, band)}>
      <div
        className={cn(
          "flex w-full flex-col gap-2 rounded-2xl bg-surface py-3 pl-4 pr-3 shadow-[0_0_0_1px_#E0DDD5,0_6px_20px_rgba(24,24,27,0.06)] dark:shadow-[0_0_0_1px_var(--color-line),0_6px_20px_rgba(0,0,0,0.3)]",
          box,
        )}
      >
        {children}
        <div className="flex items-center justify-end gap-1.5">{controls}</div>
      </div>
    </div>
  );
}

/** The textarea grows with its text to about 10 lines, then scrolls. */
const MAX_BOX_HEIGHT = 218;

/**
 * The message box of a job and of an Ask more conversation: Chat's, without the model control.
 * `Enter` sends and `Shift+Enter` makes a new line. With `onStop`, `Stop` stands beside `Send`.
 */
export function MessageComposer({
  text,
  onTextChange,
  onSend,
  placeholder,
  disabled = false,
  sendDisabled,
  onStop,
  areaRef,
}: {
  text: string;
  onTextChange: (text: string) => void;
  onSend: () => void;
  placeholder: string;
  /** The textarea itself takes no text (a job that is still starting). */
  disabled?: boolean;
  sendDisabled: boolean;
  onStop?: () => void;
  areaRef?: RefObject<HTMLTextAreaElement | null>;
}) {
  const own = useRef<HTMLTextAreaElement>(null);
  const area = areaRef ?? own;
  useLayoutEffect(() => {
    const box = area.current;
    if (!box) return;
    box.style.height = "auto";
    if (box.scrollHeight > 0) box.style.height = `${Math.min(box.scrollHeight + 2, MAX_BOX_HEIGHT)}px`;
  }, [text, area]);
  return (
    <ComposerFrame
      column="jobs"
      controls={
        <>
          {onStop && (
            <Button variant="outline" size="icon" className="size-[34px] rounded-[10px]" aria-label="Stop the agent" title="Stop the agent" onClick={onStop}>
              <Square aria-hidden />
            </Button>
          )}
          <Button size="icon" aria-label="Send" className="size-[34px] rounded-[10px]" aria-disabled={sendDisabled} onClick={onSend}>
            <ArrowUp aria-hidden />
          </Button>
        </>
      }
    >
      <TaggedTextarea
        ref={area}
        aria-label="Message the agent"
        placeholder={placeholder}
        rows={2}
        className="max-h-60 min-h-[3.75rem] bg-transparent"
        areaClassName="resize-none border-transparent focus-visible:ring-0 focus-visible:ring-offset-0"
        value={text}
        disabled={disabled}
        onChange={(event) => onTextChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            onSend();
          }
        }}
      />
    </ComposerFrame>
  );
}

/** The box of a sub-agent conversation: the layout of the message box, with nothing to send. */
export function DisabledComposer() {
  return (
    <ComposerFrame
      column="jobs"
      controls={
        <Button size="icon" aria-label="Send" className="size-[34px] rounded-[10px]" disabled>
          <ArrowUp aria-hidden />
        </Button>
      }
    >
      <Textarea
        aria-label="Message the agent"
        placeholder="Sub-agents take no messages."
        rows={2}
        className="max-h-60 min-h-[3.75rem] resize-none border-transparent bg-transparent shadow-none"
        value=""
        readOnly
        disabled
      />
    </ComposerFrame>
  );
}

/** What stands in place of the message box of a conversation that cannot take messages: a line centred in the column, over the fade. */
export function ComposerLine({ children }: { children: ReactNode }) {
  return (
    <div className={cn("flex justify-center", FADE, FRAMES.jobs.band)}>
      <div className={cn("flex w-full items-center justify-center gap-2 text-sm text-muted-foreground", FRAMES.jobs.box)}>{children}</div>
    </div>
  );
}
