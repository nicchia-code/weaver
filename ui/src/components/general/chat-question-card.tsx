import { MessagesSquare } from "lucide-react";
import { useCallback, useEffect, useRef, type FocusEvent } from "react";

import type { ActionInput } from "@/bindings/ActionInput";
import { Markdown } from "@/components/markdown";
import { QuestionIndex, StatusIcon } from "@/components/questions/question-index";
import { describeAnswer, positionOf, type RoundData } from "@/components/questions/question-model";
import { NO_HANDLERS, onQuestionKeys, QuestionPane, type KeyHandlers } from "@/components/questions/question-pane";
import { SummaryPane } from "@/components/questions/summary-pane";
import { Badge } from "@/components/ui/badge";

/** How a closed card was received by the agent: what its pill says. */
export function closingOf(card: Pick<RoundData, "questions">): { text: string; variant: "success" | "warning" | "muted" } {
  const reached = card.questions.filter((question) => question.status !== "missing").length;
  if (reached === card.questions.length) return { text: "Answered", variant: "success" };
  if (reached > 0) return { text: "Partly answered", variant: "warning" };
  return { text: "Not answered", variant: "muted" };
}

/** What reached the agent, read-only: each question with its answer, `Skipped`, or `Not answered`. */
function ClosedSummary({ card }: { card: RoundData }) {
  return (
    <ol aria-label="Answers sent" className="flex flex-col gap-3 p-[18px]">
      {card.questions.map((question, index) => {
        const answer = question.status === "answered" && question.confirmed ? describeAnswer(question, question.confirmed) : null;
        return (
          <li key={question.id} className="flex items-start gap-3">
            <span className="mt-0.5 text-sm text-muted-foreground">{index + 1}.</span>
            <div className="min-w-0 flex-1">
              <Markdown className="text-sm font-medium">{question.question}</Markdown>
              {answer ? (
                <div className="mt-1 flex flex-col gap-1 text-sm">
                  {answer.labels.length > 0 ? <p className="font-bold">{answer.labels.join(", ")}</p> : null}
                  {answer.text ? <p className="whitespace-pre-wrap text-muted-foreground">{answer.text}</p> : null}
                </div>
              ) : question.status === "skipped" ? (
                <p className="mt-1 flex items-center gap-1 text-sm text-muted-foreground">
                  <StatusIcon status="skipped" />
                  Skipped
                </p>
              ) : (
                <p className="mt-1 text-sm text-muted-foreground">Not answered</p>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

interface CardProps {
  /** The questions of the card, what the developer confirmed or skipped, and the question in view. */
  card: RoundData;
  /** `true` once the card was sent or closed by a message: it is then the read-only summary. */
  closed: boolean;
  /** What the developer does in the active card; the backend keeps it. */
  act: (action: ActionInput) => void;
  /** `Ask more` on a question of the card: a function, or `false` while it cannot be asked. */
  askMore: false | ((index: number) => void);
  /** The answers wait to be delivered (the agent starts or reloads): `Starting Weaver…` shows under the card. */
  starting?: boolean;
}

/**
 * A card of questions of Chat (`requirements/ui#question-round/Active in General`): in the frame of
 * Chat's Proposal card, one question at a time with the numbered dots, then the Summary and
 * `Send to agent`; once closed, the read-only summary of what reached the agent with a pill. It never
 * takes the focus by itself: the panes focus the question only while the focus is already in the
 * card, and `1`–`9` and `Ctrl/Cmd+Enter` work only from inside it.
 */
export function ChatQuestionCard({ card, closed, act, askMore, starting = false }: CardProps) {
  const root = useRef<HTMLElement>(null);
  const handlers = useRef<KeyHandlers>(NO_HANDLERS);
  // Whether the focus is in the card: a button that was pressed to move to another question is gone
  // by then (it leaves the focus nowhere), so the focus stays "inside" until it goes elsewhere.
  const inside = useRef(false);
  const shouldFocus = useCallback(() => inside.current, []);

  useEffect(() => {
    const away = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) inside.current = false;
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  }, []);

  const closing = closingOf(card);
  const position = card.position;
  const go = (next: Extract<ActionInput, { kind: "go" }>["position"]) => act({ kind: "go", position: next });

  return (
    <>
      <article
        ref={root}
        aria-label="Questions from Weaver"
        data-chat-card={card.id}
        onKeyDown={(event) => onQuestionKeys(event, handlers)}
        onPointerDownCapture={() => {
          inside.current = true;
        }}
        onFocusCapture={() => {
          inside.current = true;
        }}
        onBlurCapture={(event: FocusEvent<HTMLElement>) => {
          if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) inside.current = false;
        }}
        className="flex flex-col overflow-hidden rounded-[14px] bg-surface shadow-[0_0_0_1px_#E2DDF0,0_4px_14px_rgba(91,74,153,0.07)] dark:shadow-[0_0_0_1px_var(--color-line),0_4px_14px_rgba(0,0,0,0.25)]"
      >
        <div className="flex flex-col gap-3 border-b border-[#ECE7F6] bg-lavender-tint/40 px-[18px] py-4 dark:border-line">
          <div className="flex items-start gap-3">
            <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-[9px] bg-lavender-tint text-lavender-ink">
              <MessagesSquare className="size-4" />
            </span>
            <div className="flex min-w-0 flex-1 flex-col gap-1">
              <h3 className="text-base font-semibold tracking-title">Questions from Weaver</h3>
              {closed ? null : <p className="text-[13px] text-muted-foreground">{positionOf(card)}</p>}
            </div>
            {closed ? <Badge variant={closing.variant}>{closing.text}</Badge> : null}
          </div>
          {closed ? null : <QuestionIndex round={card} compact go={go} />}
        </div>
        {closed ? (
          <ClosedSummary card={card} />
        ) : position.kind === "summary" ? (
          <SummaryPane key={`${card.id}:summary`} round={card} act={act} shouldFocus={shouldFocus} handlers={handlers} />
        ) : (
          <QuestionPane
            key={`${card.id}:${position.index}`}
            round={card}
            index={position.index}
            act={act}
            askMore={askMore}
            shouldFocus={shouldFocus}
            handlers={handlers}
          />
        )}
      </article>
      {starting ? <p className="text-right text-xs text-muted-foreground">Starting Weaver…</p> : null}
    </>
  );
}
