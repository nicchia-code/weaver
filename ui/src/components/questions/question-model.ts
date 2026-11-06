// What the question round overlay works out from a round, with no React in it.
import type { DraftView } from "@/bindings/DraftView";
import type { OriginView } from "@/bindings/OriginView";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import type { QuestionView } from "@/bindings/QuestionView";

/** `1 round`, `2 rounds`. */
export function plural(count: number, one: string, many: string = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** The header of the overlay: who asks. */
export function headerOf(origin: OriginView): string {
  switch (origin.kind) {
    case "agent":
      return `Questions from the agent · ${origin.jobName}`;
    case "chat":
      return "Questions from the agent · Chat";
    case "drift":
      return `Question about ${origin.drift} · ${origin.title}`;
  }
}

/** `Question 2 of 3`, or `Summary` once the developer reviews them. */
export function positionOf(round: QuestionRoundView): string {
  return round.position.kind === "summary" ? "Summary" : `Question ${round.position.index + 1} of ${round.questions.length}`;
}

/** `+2 more rounds waiting`, or nothing when no round waits. */
export function waitingOf(round: QuestionRoundView): string | null {
  return round.waiting > 0 ? `+${round.waiting} more ${round.waiting === 1 ? "round" : "rounds"} waiting` : null;
}

/** The options chosen after the developer picked `option`: one at most for a single choice, which a second pick clears. */
export function picked(question: QuestionView, selected: number[], option: number): number[] {
  if (question.multi) {
    return selected.includes(option) ? selected.filter((i) => i !== option) : [...selected, option].sort((a, b) => a - b);
  }
  return selected.includes(option) ? [] : [option];
}

/** An answer needs an option or text with something in it. */
export function canConfirm(selected: readonly number[], text: string): boolean {
  return selected.length > 0 || text.trim() !== "";
}

/** Questions with neither a confirmed answer nor a skip. */
export function missingCount(round: QuestionRoundView): number {
  return round.questions.filter((question) => question.status === "missing").length;
}

/** What the summary says of a confirmed answer: the labels chosen, and the text when there is some. */
export function describeAnswer(question: QuestionView, answer: DraftView): { labels: string[]; text: string } {
  return { labels: answer.selected.map((i) => question.options[i]?.label ?? ""), text: answer.text.trim() };
}

/** The status of a question as text. */
export function statusText(status: QuestionView["status"]): string {
  return status === "answered" ? "Answered" : status === "skipped" ? "Skipped" : "Missing";
}
