import { MessageCircleQuestion } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MutableRefObject } from "react";

import type { ActionInput } from "@/bindings/ActionInput";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import type { QuestionView } from "@/bindings/QuestionView";
import { GuardedButton } from "@/components/guarded-button";
import { useOptionalAsks } from "@/components/ask/use-asks";
import { Markdown } from "@/components/markdown";
import { canConfirm, picked } from "@/components/questions/question-model";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter } from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

/** Time after the last keystroke by which the text reaches the backend. */
export const DRAFT_DEBOUNCE_MS = 300;

/** What the overlay calls on a key: the primary button, and an option by its digit. */
export interface KeyHandlers {
  primary: () => void;
  digit: (option: number) => void;
}

export const NO_HANDLERS: KeyHandlers = { primary: () => {}, digit: () => {} };

interface OptionsProps {
  question: QuestionView;
  selected: number[];
  onPick: (option: number) => void;
}

/**
 * The options of a question: a radio group (single choice) or a group of checkboxes, each a full
 * row with its key hint. A click on a selected single option clears it. The arrows move the focus
 * inside the group; Space and Enter pick.
 */
function Options({ question, selected, onPick }: OptionsProps) {
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const [details, setDetails] = useState<ReadonlySet<number>>(new Set());
  const [focused, setFocused] = useState(0);
  const tabbable = selected[0] ?? focused;
  const role = question.multi ? "checkbox" : "radio";

  function move(event: KeyboardEvent, from: number) {
    const last = question.options.length - 1;
    let to: number | null = null;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") to = from === last ? 0 : from + 1;
    if (event.key === "ArrowUp" || event.key === "ArrowLeft") to = from === 0 ? last : from - 1;
    if (event.key === "Home") to = 0;
    if (event.key === "End") to = last;
    if (to !== null) {
      event.preventDefault();
      setFocused(to);
      rows.current[to]?.focus();
    } else if (event.key === " " || (event.key === "Enter" && !event.ctrlKey && !event.metaKey)) {
      event.preventDefault();
      onPick(from);
    }
  }

  return (
    <div role={question.multi ? "group" : "radiogroup"} aria-label="Options" className="flex flex-col gap-2">
      {question.options.map((option, index) => {
        const chosen = selected.includes(index);
        const open = details.has(index);
        return (
          <div key={option.label} className={cn("rounded-md border", chosen && "border-primary bg-accent/50")}>
            <div
              ref={(element) => {
                rows.current[index] = element;
              }}
              role={role}
              aria-checked={chosen}
              tabIndex={index === tabbable ? 0 : -1}
              onClick={() => onPick(index)}
              onFocus={() => setFocused(index)}
              onKeyDown={(event) => move(event, index)}
              className="flex cursor-pointer items-start gap-3 rounded-md p-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span
                aria-hidden
                className={cn(
                  "mt-0.5 flex size-4 shrink-0 items-center justify-center border border-input",
                  question.multi ? "rounded-sm" : "rounded-full",
                  chosen && "border-primary bg-primary",
                )}
              >
                {chosen ? <span className={cn("bg-primary-foreground", question.multi ? "size-2 rounded-[1px]" : "size-1.5 rounded-full")} /> : null}
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  {index < 9 ? (
                    <kbd aria-hidden className="rounded-sm border bg-muted px-1.5 text-xs text-muted-foreground">
                      {index + 1}
                    </kbd>
                  ) : null}
                  <span className="font-bold">{option.label}</span>
                  {index === question.recommended ? <Badge variant="lavender">Recommended</Badge> : null}
                </div>
                <Markdown className="mt-1 text-sm text-muted-foreground">{option.description}</Markdown>
              </div>
            </div>
            {option.details ? (
              <div className="px-3 pb-3 pl-10">
                <button
                  type="button"
                  aria-expanded={open}
                  onClick={() =>
                    setDetails((current) => {
                      const next = new Set(current);
                      if (!next.delete(index)) next.add(index);
                      return next;
                    })
                  }
                  className="rounded-sm text-xs font-medium text-primary underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {open ? "Hide details" : "Show details"}
                </button>
                {open ? <Markdown className="mt-2 text-sm">{option.details}</Markdown> : null}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

/** The textarea grows with its text, up to 10 lines. */
function rowsFor(text: string): number {
  return Math.min(10, Math.max(3, text.split("\n").length));
}

interface PaneProps {
  round: QuestionRoundView;
  index: number;
  act: (action: ActionInput) => void;
  /** Opens the confirmation of `Skip all`. */
  askSkipAll: () => void;
  handlers: MutableRefObject<KeyHandlers>;
}

/**
 * One question: its text, context and options, the free answer, and the footer. Everything the
 * developer does reaches the backend at once, except the free text, which reaches it 300 ms after
 * the last keystroke, on blur and before any other action.
 */
export function QuestionPane(props: PaneProps) {
  const question = props.round.questions[props.index];
  return question ? <QuestionBody {...props} question={question} /> : null;
}

function QuestionBody({ round, index, question, act, askSkipAll, handlers }: PaneProps & { question: QuestionView }) {
  const last = index === round.questions.length - 1;
  const [text, setText] = useState(question.draft.text);
  const textNow = useRef(text);
  const sentText = useRef(question.draft.text);
  const selectedNow = useRef(question.draft.selected);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const title = useRef<HTMLDivElement>(null);
  const actNow = useRef(act);
  actNow.current = act;
  selectedNow.current = question.draft.selected;
  const asks = useOptionalAsks();

  /**
   * `Ask more` on the question in view: a new conversation opens, and the round is reduced to its
   * bar with the question in view, the choices and the written text as they were. Nothing changes
   * when the question can no longer be asked about.
   */
  function askMore() {
    flush();
    void asks?.askMore({ kind: "question", round: round.id, index }).then((id) => {
      if (id !== null) act({ kind: "minimize", minimized: true });
    });
  }

  /** Sends the free text now when the backend does not hold it yet. */
  function flush() {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    if (textNow.current === sentText.current) return;
    sentText.current = textNow.current;
    act({ kind: "draft", index, selected: selectedNow.current, text: textNow.current });
  }
  const flushNow = useRef(flush);
  flushNow.current = flush;

  // The focus goes to the text of the question each time a question is shown.
  useEffect(() => title.current?.focus(), []);
  // Leaving the question never loses what was typed.
  useEffect(() => () => flushNow.current(), []);
  // A question the backend changed under the textarea (another round of the same index) starts over.
  useEffect(() => {
    if (question.draft.text !== sentText.current && timer.current === null) {
      sentText.current = question.draft.text;
      textNow.current = question.draft.text;
      setText(question.draft.text);
    }
  }, [question.draft.text]);

  function pick(option: number) {
    if (option >= question.options.length) return;
    sentText.current = textNow.current;
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    act({ kind: "draft", index, selected: picked(question, selectedNow.current, option), text: textNow.current });
  }

  function onText(value: string) {
    setText(value);
    textNow.current = value;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => flushNow.current(), DRAFT_DEBOUNCE_MS);
  }

  const answerable = canConfirm(question.draft.selected, text);
  const next = last ? ({ kind: "summary" } as const) : ({ kind: "question", index: index + 1 } as const);

  function confirm() {
    if (!answerable) return;
    flush();
    act({ kind: "confirm", index });
    act({ kind: "go", position: next });
  }

  function skip() {
    flush();
    act({ kind: "skip", index });
    act({ kind: "go", position: next });
  }

  useLayoutEffect(() => {
    handlers.current = { primary: confirm, digit: pick };
  });

  return (
    <>
      <DialogBody className="flex flex-col gap-4 pr-1">
        <div ref={title} tabIndex={-1} className="flex flex-col gap-2 rounded-md focus-visible:outline-none">
          <div className="flex items-start gap-2">
            <Markdown className="min-w-0 flex-1 text-base font-medium" holderHeadingLevel={2}>
              {question.question}
            </Markdown>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" className="size-7 shrink-0" aria-label="Ask more about this question" onClick={askMore}>
                  <MessageCircleQuestion aria-hidden />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Ask an agent about this question</TooltipContent>
            </Tooltip>
          </div>
          {question.context ? <Markdown className="text-sm text-muted-foreground">{question.context}</Markdown> : null}
        </div>
        <Options key={`${round.id}:${index}`} question={question} selected={question.draft.selected} onPick={pick} />
        <div className="flex flex-col gap-1.5">
          <label htmlFor={`free-answer-${index}`} className="text-sm font-medium">
            Your answer (optional)
          </label>
          <Textarea
            id={`free-answer-${index}`}
            value={text}
            rows={rowsFor(text)}
            placeholder="Write a free answer or a note for the agent"
            onChange={(event) => onText(event.target.value)}
            onBlur={flush}
            className="resize-none"
          />
        </div>
      </DialogBody>
      <DialogFooter className="items-center sm:justify-between">
        <Button
          variant="outline"
          disabled={index === 0}
          onClick={() => {
            flush();
            act({ kind: "go", position: { kind: "question", index: index - 1 } });
          }}
        >
          Back
        </Button>
        <div className="flex flex-col-reverse gap-2 sm:flex-row">
          <Button variant="outline" onClick={askSkipAll}>
            Skip all
          </Button>
          <Button variant="outline" onClick={skip}>
            Skip
          </Button>
          <GuardedButton reasons={[!answerable && "Choose an option or write an answer."]} onClick={confirm}>
            {last ? "Review" : "Next"}
          </GuardedButton>
        </div>
      </DialogFooter>
    </>
  );
}
