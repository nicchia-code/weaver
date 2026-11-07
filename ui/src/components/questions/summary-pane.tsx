import { useEffect, useLayoutEffect, useRef, type MutableRefObject } from "react";
import { Pencil, TriangleAlert } from "lucide-react";

import type { ActionInput } from "@/bindings/ActionInput";
import { GuardedButton } from "@/components/guarded-button";
import { Markdown } from "@/components/markdown";
import { StatusIcon } from "@/components/questions/question-index";
import { describeAnswer, missingCount, plural, statusText, type RoundData } from "@/components/questions/question-model";
import type { KeyHandlers } from "@/components/questions/question-pane";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter } from "@/components/ui/dialog";

interface SummaryProps {
  round: RoundData;
  act: (action: ActionInput) => void;
  /** Opens the confirmation of `Skip all`; a summary without it has no `Skip all`. */
  askSkipAll?: () => void;
  /** Whether the summary takes the focus when it is shown; by default it always does. */
  shouldFocus?: () => boolean;
  handlers: MutableRefObject<KeyHandlers>;
}

const ALWAYS = () => true;

/** Every answer of the round, with `Edit`, and the send. */
export function SummaryPane({ round, act, askSkipAll, shouldFocus = ALWAYS, handlers }: SummaryProps) {
  const missing = missingCount(round);
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (shouldFocus()) title.current?.focus();
  }, [shouldFocus]);

  function send() {
    if (missing === 0) act({ kind: "send" });
  }

  useLayoutEffect(() => {
    handlers.current = { primary: send, digit: () => {} };
  });

  return (
    <>
      <DialogBody className="flex flex-col gap-3 pr-1">
        <h3 ref={title} tabIndex={-1} className="text-base font-medium focus-visible:outline-none">
          Review your answers
        </h3>
        <ol className="flex flex-col gap-3">
          {round.questions.map((question, index) => {
            const answer = question.confirmed ? describeAnswer(question, question.confirmed) : null;
            return (
              <li key={question.id} className="flex items-start gap-3 rounded-lg p-3 shadow-ring">
                <span className="mt-0.5 text-sm text-muted-foreground">{index + 1}.</span>
                <div className="min-w-0 flex-1">
                  <Markdown className="line-clamp-3 text-sm font-medium">{question.question}</Markdown>
                  {question.status === "answered" && answer ? (
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
                    <p className="mt-1 flex items-center gap-1 text-sm font-medium text-peach-ink">
                      <TriangleAlert aria-hidden className="size-4 shrink-0" />
                      {statusText("missing")}
                    </p>
                  )}
                </div>
                <Button variant="ghost" size="sm" aria-label={`Edit question ${index + 1}`} onClick={() => act({ kind: "go", position: { kind: "question", index } })}>
                  <Pencil aria-hidden />
                  Edit
                </Button>
              </li>
            );
          })}
        </ol>
      </DialogBody>
      <DialogFooter className="items-center sm:justify-between">
        <Button variant="outline" onClick={() => act({ kind: "go", position: { kind: "question", index: round.questions.length - 1 } })}>
          Back
        </Button>
        <div className="flex flex-col-reverse gap-2 sm:flex-row">
          {askSkipAll ? (
            <Button variant="outline" onClick={askSkipAll}>
              Skip all
            </Button>
          ) : null}
          <GuardedButton reasons={[missing > 0 && `Answer or skip ${plural(missing, "more question")} to send.`]} onClick={send}>
            Send to agent
          </GuardedButton>
        </div>
      </DialogFooter>
    </>
  );
}
