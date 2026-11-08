import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { AskRow } from "@/bindings/AskRow";
import type { Entry } from "@/bindings/Entry";
import { MESSAGE_PLACEHOLDER, PLACEHOLDERS } from "@/components/ask/ask-model";
import { useAskConversation } from "@/components/ask/use-ask-conversation";
import { ConversationEntries, FollowScroller, MessageComposer } from "@/components/jobs/conversation-ui";
import { useOptionalQuestionRound } from "@/components/questions/use-question-round";
import { Button } from "@/components/ui/button";
import { useOmp } from "@/hooks/use-omp";
import { call, isAppError } from "@/lib/backend";

/** The index of the last message of the developer that omp has not received yet, or -1. */
function lastQueued(entries: readonly Entry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index];
    if (entry?.kind === "developer" && entry.delivery.status === "queued") return index;
  }
  return -1;
}

/**
 * The conversation of an Ask more conversation, rendered as a job's: messages, tools, sub-agents,
 * permission cards and queued messages, but no instruction, notes, end line or finished line.
 * Empty it shows only the message box. `focus` grows when the message box must take the focus.
 */
export function AskConversation({ ask, focus }: { ask: AskRow; focus: number }) {
  const { entries, arrivals } = useAskConversation(ask.id);
  const { status } = useOmp();
  const round = useOptionalQuestionRound();
  const [text, setText] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const box = useRef<HTMLTextAreaElement>(null);

  // The message box takes the focus when the conversation opens (after anything that closes a dialog).
  useEffect(() => {
    if (focus === 0) return;
    box.current?.focus();
    const timer = setTimeout(() => box.current?.focus(), 0);
    return () => clearTimeout(timer);
  }, [focus]);

  const working = ask.status !== "idle";
  const waiting = ask.starting ? lastQueued(entries) : -1;
  const backToQuestions = ask.round !== null && round?.round?.id === ask.round;
  const reply = (action: Promise<unknown>) =>
    action.catch((error: unknown) => setProblem(isAppError(error) ? error.message : "The command failed."));

  const send = () => {
    const message = text.trim();
    if (!message) return;
    setText("");
    setProblem(null);
    void reply(call("ask_send_message", { askId: ask.id, text: message }));
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      {backToQuestions && (
        <div className="mx-auto w-full max-w-[860px] shrink-0 px-7 pt-4">
          <Button variant="outline" size="sm" onClick={() => round?.act({ kind: "minimize", minimized: false })}>
            <ArrowLeft aria-hidden />
            Back to questions
          </Button>
        </div>
      )}
      <FollowScroller version={entries}>
        <ConversationEntries
          entries={entries}
          arrivals={arrivals}
          working={working}
          latest
          answer={(request, option) => void reply(call("ask_answer_permission", { askId: ask.id, request, option }))}
          after={(_, index) => index === waiting && <p className="text-right text-xs text-muted-foreground">Starting Weaver…</p>}
        />
      </FollowScroller>
      {status?.state === "missing" && (
        <div role="status" className="mx-auto flex w-full max-w-[860px] items-center gap-2 px-7 pb-2 text-sm text-rose-ink">
          The agent is not installed.
          <Button variant="outline" size="sm" onClick={() => void call("omp_install").catch(() => {})}>
            Install the agent
          </Button>
        </div>
      )}
      {problem && (
        <p role="alert" className="mx-auto w-full max-w-[860px] px-7 pb-2 text-xs text-rose-ink">
          {problem}
        </p>
      )}
      <MessageComposer
        areaRef={box}
        text={text}
        onTextChange={setText}
        onSend={send}
        placeholder={entries.length === 0 ? PLACEHOLDERS[ask.kind] : MESSAGE_PLACEHOLDER}
        sendDisabled={text.trim() === ""}
        onStop={working ? () => void reply(call("ask_stop", { askId: ask.id })) : undefined}
      />
    </div>
  );
}
