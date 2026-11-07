import { ArrowLeft, ArrowUp, Square } from "lucide-react";
import { Fragment, useEffect, useRef, useState } from "react";

import type { AskRow } from "@/bindings/AskRow";
import type { Entry } from "@/bindings/Entry";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { MESSAGE_PLACEHOLDER, PLACEHOLDERS } from "@/components/ask/ask-model";
import { useAskConversation } from "@/components/ask/use-ask-conversation";
import { EntryView, FollowScroller } from "@/components/jobs/jobs-view";
import { useChatCards } from "@/components/general/use-chat-cards";
import { useProject } from "@/components/project-context";
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

/** The element of a card of questions of the Chat: the Chat view marks its card with the card's identity. */
function cardElement(card: string): Element | null {
  return document.querySelector(`[data-chat-card="${CSS.escape(card)}"]`);
}

/**
 * Shows the Chat view with `card` in view, where the developer left it: the card keeps its question
 * in view, its choices and its text, so only the scroll moves. The view may still be hidden for a
 * frame, and the card is looked for until it is found.
 */
function revealCard(card: string, showView: (view: "general") => void): void {
  showView("general");
  let frames = 0;
  const find = () => {
    const element = cardElement(card);
    if (element) element.scrollIntoView({ block: "start" });
    else if ((frames += 1) < 30) requestAnimationFrame(find);
  };
  requestAnimationFrame(find);
}

/**
 * `Back to questions` of a conversation opened from the question in view of a card of Chat: shown
 * only while that card is still the active one, wherever the Chat view is.
 */
function BackToCard({ projectId, card }: { projectId: string; card: string }) {
  const { cards } = useChatCards(projectId);
  const { showView } = useProject();
  if (!cards.some((view) => view.id === card && view.state === "active")) return null;
  return (
    <div className="border-b px-4 py-3">
      <Button variant="outline" size="sm" onClick={() => revealCard(card, showView)}>
        <ArrowLeft aria-hidden />
        Back to questions
      </Button>
    </div>
  );
}

/**
 * The conversation of an Ask more conversation, rendered as a job's: messages, tools, sub-agents,
 * permission cards and queued messages, but no instruction, notes, end line or finished line.
 * Empty it shows only the message box. `focus` grows when the message box must take the focus.
 */
export function AskConversation({ ask, focus }: { ask: AskRow; focus: number }) {
  const entries = useAskConversation(ask.id);
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
      {ask.card !== null && <BackToCard projectId={ask.projectId} card={ask.card} />}
      {backToQuestions && (
        <div className="border-b px-4 py-3">
          <Button variant="outline" size="sm" onClick={() => round?.act({ kind: "minimize", minimized: false })}>
            <ArrowLeft aria-hidden />
            Back to questions
          </Button>
        </div>
      )}
      <FollowScroller version={entries}>
        {entries.map((entry, index) => (
          <Fragment key={index}>
            <EntryView
              entry={entry}
              answer={(request, option) => void reply(call("ask_answer_permission", { askId: ask.id, request, option }))}
            />
            {index === waiting && <p className="text-right text-xs text-muted-foreground">Starting Weaver…</p>}
          </Fragment>
        ))}
      </FollowScroller>
      {status?.state === "missing" && (
        <div role="status" className="flex items-center gap-2 border-t p-3 text-sm text-rose-ink">
          The agent is not installed.
          <Button variant="outline" size="sm" onClick={() => void call("omp_install").catch(() => {})}>
            Install the agent
          </Button>
        </div>
      )}
      {problem && (
        <p role="alert" className="border-t px-3 pt-2 text-xs text-rose-ink">
          {problem}
        </p>
      )}
      <div className="flex items-end gap-2 border-t p-3">
        <TaggedTextarea
          ref={box}
          aria-label="Message the agent"
          placeholder={entries.length === 0 ? PLACEHOLDERS[ask.kind] : MESSAGE_PLACEHOLDER}
          rows={1}
          className="max-h-60 min-h-9 flex-1"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              send();
            }
          }}
        />
        <Button size="icon" aria-label="Send" aria-disabled={text.trim() === ""} onClick={send}>
          <ArrowUp aria-hidden />
        </Button>
        {working && (
          <Button variant="outline" size="icon" aria-label="Stop the agent" title="Stop the agent" onClick={() => void reply(call("ask_stop", { askId: ask.id }))}>
            <Square aria-hidden />
          </Button>
        )}
      </div>
    </div>
  );
}
