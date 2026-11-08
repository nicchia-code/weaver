import { MessageCircleQuestion } from "lucide-react";

import type { AskTarget } from "@/bindings/AskTarget";
import { useBoardActions } from "@/components/board/board-ui";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useViews } from "@/components/views/views-state";

/**
 * `Ask more` in the footer of a details dialog of the Board or of a History entry: an outline
 * button, always enabled, with the icon on the left. Pressing it opens a conversation about the
 * card or the entry; the dialog stays as it was. `name` is the short number (the permanent
 * identity when the card has none, and the entry's identity for a History entry).
 */
export function AskMoreButton({
  target,
  noun,
  name,
}: {
  target: AskTarget;
  noun: "Proposal" | "Task" | "Drift" | "History entry";
  name: string;
}) {
  const { askMore } = useBoardActions();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="outline" size="sm" aria-label={`Ask more about ${name}`} onClick={() => void askMore?.(target)}>
          <MessageCircleQuestion aria-hidden />
          Ask more
        </Button>
      </TooltipTrigger>
      <TooltipContent>Ask an agent about this {noun}</TooltipContent>
    </Tooltip>
  );
}

/**
 * `Ask more` in the header of the Requirements view, between the search and `Ask for a change`: a
 * ghost button with the icon and the text, always visible and enabled, about the Requirements as a
 * whole. It sends only the overlay Proposal in view, if any; never a section, an Area, the focus or
 * `Only these`. The view stays as it was.
 */
export function AskRequirementsButton() {
  const { askMore } = useBoardActions();
  const { snapshot } = useViews();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          aria-label="Ask more about the Requirements"
          onClick={() => void askMore?.({ kind: "requirements", overlay: snapshot?.overlay ?? null })}
        >
          <MessageCircleQuestion aria-hidden />
          Ask more
        </Button>
      </TooltipTrigger>
      <TooltipContent>Ask an agent about the Requirements</TooltipContent>
    </Tooltip>
  );
}
