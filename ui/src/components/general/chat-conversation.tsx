import { Clock, ChevronDown } from "lucide-react";
import { useState } from "react";

import type { Entry } from "@/bindings/Entry";
import { EntryView } from "@/components/jobs/jobs-view";
import { runningText } from "@/components/jobs/format";
import { Logo } from "@/components/logo";
import { Markdown } from "@/components/markdown";
import { TONE_DOT } from "@/lib/tones";
import { cn } from "@/lib/utils";

import { clockText, workText, type ChatItem } from "./chat-model";

type Answer = (request: number, option: string) => void;

/** The sender row of the agent: the logo at 24 px, `Weaver` and the time. */
export function SenderRow({ at }: { at: number | null }) {
  return (
    <div className="flex items-center gap-2.5">
      <Logo size={24} />
      <span className="text-[13px] font-semibold">Weaver</span>
      {at === null ? null : <span className="text-xs text-muted-foreground">{clockText(at)}</span>}
    </div>
  );
}

/** A message of the developer: a box on Paper, right-aligned, at most 560 px wide. */
function DeveloperMessage({ entry }: { entry: Extract<Entry, { kind: "developer" }> }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[560px] whitespace-pre-wrap rounded-2xl rounded-br-[4px] bg-paper px-4 py-3 text-sm leading-[1.55]">
        {entry.text}
        {entry.delivery.status === "queued" && (
          <div className="mt-1 flex items-center gap-1 text-xs text-muted-foreground">
            <Clock aria-hidden className="size-3" />
            Queued
          </div>
        )}
        {entry.delivery.status === "not-delivered" && (
          <div className="mt-1 text-xs text-rose-ink">Not delivered: {entry.delivery.reason}.</div>
        )}
      </div>
    </div>
  );
}

/**
 * One entry of the conversation: the messages are Chat's own, every other entry renders as in a job.
 * A message made only of the answers of a card (`text` empty) is no message of the developer: the
 * card shows in its place, and only a refused delivery says so.
 */
export function ChatEntry({ entry, answer }: { entry: Entry; answer: Answer }) {
  if (entry.kind === "developer" && entry.text === "") {
    return entry.delivery.status === "not-delivered" ? (
      <p className="text-right text-xs text-rose-ink">Not delivered: {entry.delivery.reason}.</p>
    ) : null;
  }
  if (entry.kind === "developer") return <DeveloperMessage entry={entry} />;
  if (entry.kind === "agent") {
    return (
      <div className="text-[14.5px] leading-[1.65]">
        <Markdown>{entry.text}</Markdown>
      </div>
    );
  }
  return <EntryView entry={entry} answer={answer} />;
}

/**
 * A run of `Thinking` blocks and tool rows as one disclosure, closed at first: `Worked for <duration>
 * · <n> steps`, or `Working · <n> steps` with a Sky dot while it is the last thing of a turn in
 * progress. Opened, it shows the rows as a job does, sub-agent lines included.
 */
export function WorkSummary({
  item,
  entries,
  running,
  answer,
}: {
  item: Extract<ChatItem, { kind: "work" }>;
  entries: readonly Entry[];
  running: boolean;
  answer: Answer;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex min-w-[280px] max-w-full flex-col self-start rounded-xl bg-lane shadow-[inset_0_0_0_1px_var(--color-line-soft)]">
      <button
        type="button"
        aria-expanded={open}
        className="flex items-center gap-2.5 rounded-xl px-3 py-[9px] text-left text-[13px] text-ink-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        onClick={() => setOpen((value) => !value)}
      >
        {running ? (
          <span aria-hidden className={cn("size-2 shrink-0 animate-pulse rounded-full motion-reduce:animate-none", TONE_DOT.sky)} />
        ) : (
          <span aria-hidden className="flex gap-0.5">
            <span className={cn("h-3 w-[3px] rounded-sm", TONE_DOT.lavender)} />
            <span className={cn("h-3 w-[3px] rounded-sm", TONE_DOT.sky)} />
            <span className={cn("h-3 w-[3px] rounded-sm", TONE_DOT.sage)} />
          </span>
        )}
        <span className="flex-1">{workText(item, running, runningText)}</span>
        <ChevronDown aria-hidden className={cn("size-3.5 shrink-0 transition-transform duration-150", open && "rotate-180")} />
      </button>
      {open && (
        <div className="space-y-2 border-t border-line-soft px-3 pb-3 pt-2">
          {entries.slice(item.from, item.to + 1).map((entry, offset) => (
            <EntryView key={item.from + offset} entry={entry} answer={answer} />
          ))}
        </div>
      )}
    </div>
  );
}
