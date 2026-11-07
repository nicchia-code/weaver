import { Check, Circle, SkipForward } from "lucide-react";

import type { QuestionView } from "@/bindings/QuestionView";
import { statusText, type RoundData } from "@/components/questions/question-model";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/** The icon of a status: never the colour alone. */
export function StatusIcon({ status, className }: { status: QuestionView["status"]; className?: string }) {
  const Icon = status === "answered" ? Check : status === "skipped" ? SkipForward : Circle;
  return <Icon aria-hidden className={cn("size-4 shrink-0", className)} />;
}

interface IndexProps {
  round: RoundData;
  /** `true` below 600 px: a row of numbered dots with the same accessible status. */
  compact: boolean;
  go: (position: { kind: "question"; index: number } | { kind: "summary" }) => void;
}

/** First line of a Markdown text, as plain as it can be made cheaply: for one-line entries. */
function oneLine(markdown: string): string {
  return markdown.replace(/\s+/g, " ").replace(/[`*_#>]/g, "").trim();
}

/** The index of the round: one entry per question with its status, and the summary. */
export function QuestionIndex({ round, compact, go }: IndexProps) {
  const here = round.position;
  const onSummary = here.kind === "summary";
  if (compact) {
    return (
      <nav aria-label="Questions" className="flex flex-wrap items-center gap-1.5">
        {round.questions.map((question, index) => {
          const current = here.kind === "question" && here.index === index;
          const text = statusText(question.status);
          return (
            <button
              key={question.id}
              type="button"
              aria-label={`Question ${index + 1}: ${text}`}
              aria-current={current ? "step" : undefined}
              onClick={() => go({ kind: "question", index })}
              className={cn(
                "flex size-7 items-center justify-center rounded-full border text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                current ? "border-primary bg-primary text-primary-foreground" : "bg-background",
                question.status === "missing" && !current && "border-peach text-peach-ink",
              )}
            >
              {index + 1}
            </button>
          );
        })}
        <button
          type="button"
          aria-current={onSummary ? "step" : undefined}
          onClick={() => go({ kind: "summary" })}
          className={cn(
            "h-7 rounded-full border px-2.5 text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            onSummary ? "border-primary bg-primary text-primary-foreground" : "bg-background",
          )}
        >
          Summary
        </button>
      </nav>
    );
  }
  return (
    <nav aria-label="Questions" className="flex w-52 shrink-0 flex-col gap-1 overflow-y-auto border-r pr-3">
      {round.questions.map((question, index) => {
        const current = here.kind === "question" && here.index === index;
        const text = statusText(question.status);
        return (
          <Tooltip key={question.id}>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-current={current ? "step" : undefined}
                onClick={() => go({ kind: "question", index })}
                className={cn(
                  "flex flex-col gap-0.5 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                  current && "bg-accent",
                )}
              >
                <span className="truncate font-medium">
                  {index + 1}. {oneLine(question.question)}
                </span>
                <span
                  className={cn(
                    "flex items-center gap-1 text-xs text-muted-foreground",
                    question.status === "missing" && "text-peach-ink",
                  )}
                >
                  <StatusIcon status={question.status} className="size-3.5" />
                  {text}
                </span>
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{question.question}</TooltipContent>
          </Tooltip>
        );
      })}
      <button
        type="button"
        aria-current={onSummary ? "step" : undefined}
        onClick={() => go({ kind: "summary" })}
        className={cn(
          "rounded-md px-2 py-1.5 text-left text-sm font-medium hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          onSummary && "bg-accent",
        )}
      >
        Summary
      </button>
    </nav>
  );
}
