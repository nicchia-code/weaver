import { Bot, ChevronDown } from "lucide-react";
import { useState } from "react";

import type { Entry } from "@/bindings/Entry";
import { EntryView } from "@/components/jobs/entry-view";
import { runningText } from "@/components/jobs/format";
import { Logo } from "@/components/logo";
import { TONE_DOT } from "@/lib/tones";
import { cn } from "@/lib/utils";

import { clockText, workText, type ChatItem } from "./chat-model";

type Answer = (request: number, option: string) => void;

/**
 * The sender row of the agent: the logo at 24 px, `Weaver` and the time. A sub-agent's conversation
 * shows a 24 px `Bot` tile and the sub-agent's name (`agent`) instead of the logo and `Weaver`.
 */
export function SenderRow({ at, agent }: { at: number | null; agent?: string }) {
  return (
    <div className="flex items-center gap-2.5">
      {agent === undefined ? (
        <Logo size={24} />
      ) : (
        <span aria-hidden className="grid size-6 shrink-0 place-items-center rounded-md bg-lane text-ink-2 shadow-[inset_0_0_0_1px_var(--color-line-soft)]">
          <Bot className="size-3.5" />
        </span>
      )}
      <span className="min-w-0 truncate text-[13px] font-semibold">{agent ?? "Weaver"}</span>
      {at === null ? null : <span className="text-xs text-muted-foreground">{clockText(at)}</span>}
    </div>
  );
}

/**
 * A run of `Thinking` blocks and tool rows as one disclosure, closed at first: `Worked for <duration>
 * · <n> steps`, or `Working · <n> steps` (with the latest step, when the view gives it) and a Sky dot
 * while it is the last thing of a turn in progress. Opened, it shows the rows as a job does,
 * sub-agent lines included.
 */
export function WorkSummary({
  item,
  entries,
  running,
  latest = null,
  answer,
}: {
  item: Extract<ChatItem, { kind: "work" }>;
  entries: readonly Entry[];
  running: boolean;
  /** The latest step shown after `Working · <n> steps`; `null` shows none. */
  latest?: string | null;
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
        <span className="min-w-0 flex-1 truncate">{workText(item, running, runningText, latest)}</span>
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
